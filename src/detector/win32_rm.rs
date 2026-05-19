//! Win32 Restart Manager API 的真实实现。
//!
//! 使用 RAII 封装会话句柄，保证 `RmEndSession` 在任何退出路径上都被调用。
//!
//! Requirements: 9.2, 9.3, 1.7

use std::path::PathBuf;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::RestartManager::{
    RmEndSession, RmGetList, RmRegisterResources, RmStartSession, RM_PROCESS_INFO,
};

use super::restart_manager::{RestartManagerApi, RmProcessInfo};
use crate::error::RmError;

// CCH_RM_SESSION_KEY = 32 chars + NUL
const SESSION_KEY_LEN: usize = 33;

const ERROR_SUCCESS: WIN32_ERROR = WIN32_ERROR(0);
const ERROR_MORE_DATA: WIN32_ERROR = WIN32_ERROR(234);
const ERROR_ACCESS_DENIED: WIN32_ERROR = WIN32_ERROR(5);
const ERROR_FILE_NOT_FOUND: WIN32_ERROR = WIN32_ERROR(2);

/// RAII 包装：持有 RM 会话句柄，Drop 时自动调用 RmEndSession。
struct RmSession(u32);

impl RmSession {
    /// 开启新会话，返回 RAII 包装。
    fn start() -> Result<Self, RmError> {
        let mut session_handle: u32 = 0;
        let mut session_key = [0u16; SESSION_KEY_LEN];
        let err =
            unsafe { RmStartSession(&mut session_handle, 0, PWSTR(session_key.as_mut_ptr())) };
        if err != ERROR_SUCCESS {
            return Err(map_win32_err(err));
        }
        Ok(RmSession(session_handle))
    }

    fn handle(&self) -> u32 {
        self.0
    }
}

impl Drop for RmSession {
    fn drop(&mut self) {
        unsafe {
            let _ = RmEndSession(self.0);
        }
    }
}

/// 将 `WIN32_ERROR` 映射到 `RmError`，附带本地化描述。
fn map_win32_err(err: WIN32_ERROR) -> RmError {
    match err {
        ERROR_ACCESS_DENIED => RmError::AccessDenied,
        ERROR_FILE_NOT_FOUND => RmError::PathNotFound,
        other => {
            use windows::Win32::Foundation::{LocalFree, HLOCAL};
            use windows::Win32::System::Diagnostics::Debug::{
                FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
                FORMAT_MESSAGE_IGNORE_INSERTS,
            };

            let code = other.0;
            let mut buf_ptr: PWSTR = PWSTR::null();
            let len = unsafe {
                FormatMessageW(
                    FORMAT_MESSAGE_ALLOCATE_BUFFER
                        | FORMAT_MESSAGE_FROM_SYSTEM
                        | FORMAT_MESSAGE_IGNORE_INSERTS,
                    None,
                    code,
                    0,
                    PWSTR(&mut buf_ptr.0 as *mut _ as *mut u16),
                    0,
                    None,
                )
            };
            let desc = if len == 0 || buf_ptr.is_null() {
                if !buf_ptr.is_null() {
                    unsafe {
                        let _ = LocalFree(HLOCAL(buf_ptr.0 as *mut _));
                    }
                }
                format!("Win32 error 0x{:08X}", code)
            } else {
                let slice = unsafe { std::slice::from_raw_parts(buf_ptr.0, len as usize) };
                let mut s = String::from_utf16_lossy(slice).to_owned();
                unsafe {
                    let _ = LocalFree(HLOCAL(buf_ptr.0 as *mut _));
                }
                while matches!(s.chars().last(), Some('\r') | Some('\n') | Some(' ')) {
                    s.pop();
                }
                if s.is_empty() {
                    format!("Win32 error 0x{:08X}", code)
                } else {
                    s
                }
            };
            RmError::Win32 { code, desc }
        }
    }
}

/// 将 `PathBuf` 切片转为宽字符 `Vec<Vec<u16>>`，并收集 `PCWSTR` 指针。
///
/// 返回两个集合：宽字符数据的所有权（必须保持存活）以及对应的 PCWSTR 指针切片。
fn paths_to_wide(paths: &[PathBuf]) -> (Vec<Vec<u16>>, Vec<PCWSTR>) {
    use std::os::windows::ffi::OsStrExt;
    let wide_strings: Vec<Vec<u16>> = paths
        .iter()
        .map(|p| {
            let mut w: Vec<u16> = p.as_os_str().encode_wide().collect();
            w.push(0); // NUL 终止
            w
        })
        .collect();
    let ptrs: Vec<PCWSTR> = wide_strings.iter().map(|w| PCWSTR(w.as_ptr())).collect();
    (wide_strings, ptrs)
}

/// Win32 Restart Manager API 的生产实现。
pub struct Win32RestartManager;

impl RestartManagerApi for Win32RestartManager {
    fn scan(&self, paths: &[PathBuf]) -> Result<Vec<RmProcessInfo>, RmError> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        // 1. 开启 RM 会话（RAII：Drop 时自动 RmEndSession）
        let session = RmSession::start()?;

        // 2. 注册资源路径（分批，每批 ≤ 1000）
        for chunk in paths.chunks(1000) {
            let (_wide_data, ptrs) = paths_to_wide(chunk);
            let err = unsafe {
                RmRegisterResources(
                    session.handle(),
                    Some(&ptrs),
                    None, // 无服务
                    None, // 无进程
                )
            };
            if err != ERROR_SUCCESS {
                return Err(map_win32_err(err));
            }
        }

        // 3. RmGetList 双调用模式，最多重试 3 次 ERROR_MORE_DATA
        for _attempt in 0..3 {
            // 第一次调用：nProcInfo=0 取所需数量
            let mut n_needed: u32 = 0;
            let mut n_avail: u32 = 0;
            let mut reboot_reasons: u32 = 0;
            let err1 = unsafe {
                RmGetList(
                    session.handle(),
                    &mut n_needed,
                    &mut n_avail,
                    None,
                    &mut reboot_reasons,
                )
            };

            // ERROR_MORE_DATA 表示有结果但缓冲区不足，是预期路径
            if err1 != ERROR_SUCCESS && err1 != ERROR_MORE_DATA {
                return Err(map_win32_err(err1));
            }

            if n_needed == 0 {
                // 没有进程在锁定这些文件
                return Ok(Vec::new());
            }

            // 分配缓冲区并再次调用（使用 Default::default() 零初始化，避免 clippy uninit 警告）
            let capacity = n_needed as usize;
            let mut raw_infos: Vec<RM_PROCESS_INFO> = vec![RM_PROCESS_INFO::default(); capacity];
            let mut n_actual: u32 = capacity as u32;
            let mut reboot_reasons2: u32 = 0;

            let err2 = unsafe {
                RmGetList(
                    session.handle(),
                    &mut n_needed,
                    &mut n_actual,
                    Some(raw_infos.as_mut_ptr()),
                    &mut reboot_reasons2,
                )
            };

            if err2 == ERROR_MORE_DATA {
                // 两次调用之间又有进程加入，重试
                continue;
            }
            if err2 != ERROR_SUCCESS {
                return Err(map_win32_err(err2));
            }

            // 成功：转换结果
            let actual = n_actual as usize;
            let infos = raw_infos[..actual]
                .iter()
                .map(|ri| {
                    // strAppName 是 [u16; 256]，取到第一个 NUL
                    let app_name_raw = &ri.strAppName;
                    let nul_pos = app_name_raw
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(app_name_raw.len());
                    let app_name = String::from_utf16_lossy(&app_name_raw[..nul_pos]).to_owned();

                    let start_time = {
                        let ft = ri.Process.ProcessStartTime;
                        // dwLowDateTime == 0 且 dwHighDateTime == 0 表示无效
                        if ft.dwLowDateTime == 0 && ft.dwHighDateTime == 0 {
                            None
                        } else {
                            Some(ft)
                        }
                    };

                    RmProcessInfo {
                        pid: ri.Process.dwProcessId,
                        start_time,
                        app_name,
                        app_type: ri.ApplicationType.0 as u32,
                    }
                })
                .collect();
            return Ok(infos);
        }

        // 3 次重试均遇到 ERROR_MORE_DATA
        Err(RmError::Other(
            "RmGetList 持续返回 ERROR_MORE_DATA，重试耗尽".into(),
        ))
    }
}
