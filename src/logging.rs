//! 日志初始化模块。
//!
//! - 使用 `tracing-subscriber` + `tracing-appender::rolling::daily` 输出滚动日志
//!   到 `%LOCALAPPDATA%\FileLockInspector\logs\fli.log`。
//! - 默认级别 `info`；通过 `FLI_LOG=debug` 环境变量覆盖。
//! - 注册 `std::panic::set_hook`，将 panic 信息写入日志。
//! - 对外暴露 `pub fn init() -> anyhow::Result<()>`。

use std::path::PathBuf;

use anyhow::Context;
use tracing::error;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// 初始化全局 `tracing` subscriber。
///
/// 调用方（`main`）应在任何其他模块初始化之前调用此函数。
/// 返回 `Err` 仅在无法确定日志目录时发生（极罕见）。
pub fn init() -> anyhow::Result<()> {
    let log_dir = log_directory()?;
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("无法创建日志目录: {}", log_dir.display()))?;

    // 每日滚动：生成 fli.log.YYYY-MM-DD 文件
    let file_appender = rolling::daily(&log_dir, "fli.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 日志级别：优先读取 FLI_LOG 环境变量，默认 info
    let env_filter = EnvFilter::try_from_env("FLI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    // 同时输出到文件（JSON-friendly 时间戳）与 stderr（调试用）
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .init();

    // 注册 panic hook，将 panic 信息写入 tracing 日志
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string payload>".to_string()
        };

        error!(
            panic.location = %location,
            panic.payload  = %payload,
            "Application panicked"
        );
    }));

    Ok(())
}

/// 返回日志文件所在目录：`%LOCALAPPDATA%\FileLockInspector\logs`。
fn log_directory() -> anyhow::Result<PathBuf> {
    let base = dirs::data_local_dir().context("无法获取 %LOCALAPPDATA% 路径")?;
    Ok(base.join("FileLockInspector").join("logs"))
}

/// 返回日志目录（公开版本，供 cleanup_old_logs 和外部使用）。
pub fn log_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("FileLockInspector").join("logs"))
}

/// 清理超过 30 天的日志文件；当日文件超过 10 MB 时截断。
///
/// 失败静默忽略，不影响主流程。
pub fn cleanup_old_logs() {
    let dir = match log_dir() {
        Some(d) => d,
        None => return,
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            // 删除 30 天前的日志文件
            if let Ok(modified) = meta.modified() {
                if modified < cutoff {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            }
            // 当日文件超过 10 MB 时截断为空文件（避免磁盘占用过大）
            if meta.len() > 10 * 1024 * 1024 {
                let _ = std::fs::write(&path, b"");
            }
        }
    }
}
