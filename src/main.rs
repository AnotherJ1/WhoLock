#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// 脚手架阶段：大量模块尚未接入 main，允许 dead_code / unused_imports
#![allow(dead_code, unused_imports)]

mod app;
mod config;
mod detector;
mod elevation;
mod error;
mod i18n;
mod logging;
mod monitor;
mod process_info;
mod state;
mod sys_classifier;
mod terminator;
mod ui;

#[cfg(test)]
mod integration_tests;

use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;

use crate::app::FileLockInspectorApp;
use crate::state::app_state::PrivilegeLevel;
use crate::state::AppState;

fn main() {
    // 初始化日志
    if let Err(e) = logging::init() {
        eprintln!("日志初始化失败: {e}");
    }
    logging::cleanup_old_logs();

    // 检测权限
    let privilege = if elevation::is_elevated() {
        PrivilegeLevel::Elevated
    } else {
        PrivilegeLevel::Standard
    };

    // 加载配置
    let cfg = config::AppConfig::load();

    // 应用 UI 语言（默认 English）
    i18n::set_language(cfg.language);

    // 构造初始 AppState
    let mut initial_state = AppState::new_default();
    initial_state.privilege = privilege;
    initial_state.polling_interval_ms = cfg.polling_interval_ms;

    // Windows 版本检测（Requirement 9.5）
    let windows_supported = check_windows_version();
    initial_state.windows_supported = windows_supported;
    if !windows_supported {
        initial_state.last_error = Some(crate::state::app_state::UiToast::error(
            i18n::t(i18n::Key::MsgUnsupportedOs).to_string(),
        ));
    }

    let state = Arc::new(Mutex::new(initial_state));

    // 构造通道
    let (cmd_tx, cmd_rx) = unbounded::<monitor::MonitorCmd>();
    let (event_tx, event_rx) = unbounded::<monitor::ScanEvent>();

    // 启动 MonitorEngine 后台线程
    let rm = Arc::new(crate::detector::win32_rm::Win32RestartManager);
    let clock = Arc::new(crate::monitor::clock::SystemClock);
    let engine = crate::monitor::engine::MonitorEngine::new(rm, clock, cmd_rx, event_tx);
    std::thread::spawn(move || engine.run());

    // 启动 eframe
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("File Lock Inspector")
            .with_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    let state_clone = state.clone();
    if let Err(e) = eframe::run_native(
        "File Lock Inspector",
        options,
        Box::new(move |cc| {
            Ok(Box::new(FileLockInspectorApp::new(
                cc,
                state_clone,
                cmd_tx,
                event_rx,
            )))
        }),
    ) {
        eprintln!("eframe 错误: {e}");
        std::process::exit(1);
    }
}

/// 检查 Windows 版本是否 >= 10.0.17763 (Windows 10 1809)
/// 通过注册表读取 CurrentBuildNumber，避免 RtlGetVersion 的 feature 依赖问题
fn check_windows_version() -> bool {
    // 从注册表读取构建号
    // HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\CurrentBuildNumber
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let result: Option<u32> = (|| {
        use windows::core::PCWSTR;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ,
            REG_VALUE_TYPE,
        };

        let key_path: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
            .encode_utf16()
            .collect();
        let value_name: Vec<u16> = "CurrentBuildNumber\0".encode_utf16().collect();

        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let res = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key_path.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            )
        };
        if res.is_err() {
            return None;
        }

        let mut buf = [0u16; 32];
        let mut buf_size = (buf.len() * 2) as u32;
        let mut value_type = REG_VALUE_TYPE::default();
        let res = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut value_type),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut buf_size),
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        if res.is_err() {
            return None;
        }

        // buf 包含 wide 字符串形式的构建号，如 "19041"
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = OsString::from_wide(&buf[..len]);
        s.to_string_lossy().parse::<u32>().ok()
    })();

    match result {
        Some(build) => build >= 17763,
        None => true, // 读取失败时保守返回 true，不阻止使用
    }
}
