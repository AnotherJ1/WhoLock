// 错误类型定义（任务 2.4）
//
// 严格对应 design.md "Error Handling" 节：
// - `AppError`：应用顶层错误，承载 Win32 调用失败、Restart Manager 失败、路径不存在、
//   权限不足、系统不支持、内部错误，以及"自重启请求"哨兵。
// - `TerminateError`：强制结束进程相关错误，UI 层据此分发不同 Toast 与对话框。
// - `ScanFailure`：后台扫描失败的对外形态，回送给 UI 层。
// - `RmError`：Restart Manager 调用失败的细分，作为 `AppError::RestartManager` 的内层。
//
// 设计取舍：
// - `ScanFailure` / `RmError` 不直接 use `windows::core::Error`，保持纯 Rust 形态，
//   便于 Property-Based Testing 用 mock 注入任意错误码组合。
// - `TerminateError::Other(#[from] windows::core::Error)` 是允许的：force_terminate 实际
//   就是直接调 Win32，从 windows-rs 接口透出 `windows::core::Error` 是自然的。
// - 所有 Display 文案使用中文（参考 chinese-language steering），错误码以
//   `0x{:08X}` 大写零填充 8 位呈现，方便用户复制到搜索引擎排错。

use std::path::PathBuf;

use thiserror::Error;
use windows::core::PWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS,
};

// ---------------------------------------------------------------------------
// AppError：应用顶层错误
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum AppError {
    /// Win32 调用失败，附带原始错误码与本地化描述。
    /// Display 形如：`Win32 错误 0x00000005: Access is denied.`
    #[error("Win32 错误 0x{0:08X}: {1}")]
    Win32(u32, String),

    /// Restart Manager 子调用失败，借助 `#[from]` 自动从 RmError 提升。
    #[error("Restart Manager 调用失败: {0}")]
    RestartManager(#[from] RmError),

    /// 用户提交的路径在文件系统中不存在。
    #[error("路径不存在: {0}")]
    PathNotFound(PathBuf),

    /// 操作被 Windows 拒绝（通常需要管理员），UI 层据此提示提权。
    #[error("权限不足，需要管理员")]
    AccessDenied,

    /// Windows 版本低于支持范围（< 10.0.17763）。
    #[error("当前 Windows 版本不受支持")]
    UnsupportedOs,

    /// 兜底的内部错误，用于打包不便分类的逻辑错误。
    #[error("内部错误: {0}")]
    Internal(String),

    /// 哨兵变体：表示 elevation 后已成功 spawn 提权进程，主进程应优雅退出。
    /// 由 `main` 捕获并执行 `std::process::exit(0)`。
    #[error("自重启请求")]
    SelfExit,
}

impl AppError {
    /// 用 Win32 错误码构造 `AppError::Win32`，自动通过 FormatMessageW 取本地化描述。
    pub fn from_win32(code: u32) -> Self {
        AppError::Win32(code, format_message_w(code))
    }
}

// ---------------------------------------------------------------------------
// TerminateError：强制结束进程错误
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum TerminateError {
    /// OpenProcess / TerminateProcess 因权限不足失败（ERROR_ACCESS_DENIED）。
    #[error("权限不足，请以管理员身份重启程序")]
    AccessDenied,

    /// 目标进程在二次确认期间已自然退出（ERROR_INVALID_PARAMETER 通常意味此）。
    /// 业务层视为成功并触发立即刷新（需求 4.7）。
    #[error("进程已不存在")]
    AlreadyExited,

    /// PID 已被 OS 复用：OpenProcess 后查询的启动时间与扫描期记录不一致。
    /// 拒绝继续执行以避免误杀新进程。
    #[error("PID 已复用，已忽略本次操作")]
    StalePid,

    /// 命中系统进程黑名单（pid 0/4 等），调用方未经 sys_classifier 也会被此处拦截。
    #[error("系统进程禁止结束")]
    SystemProtected,

    /// `force_terminate_with_timeout` 超过 5000ms 未返回（需求 4.4）。
    #[error("操作超时，请重试")]
    Timeout,

    /// 其他 Win32 错误码（含本地化描述）。Display 形如：`Windows 错误 0x000000B7: ...`
    #[error("Windows 错误 0x{code:08X}: {desc}")]
    Win32 { code: u32, desc: String },

    /// 透传 windows-rs 抛出的 Result 错误。强制结束实际调 Win32，自然出现此变体。
    #[error(transparent)]
    Other(#[from] windows::core::Error),
}

impl TerminateError {
    /// 用 Win32 错误码构造 `TerminateError::Win32`，自动取本地化描述。
    pub fn from_win32(code: u32) -> Self {
        TerminateError::Win32 {
            code,
            desc: format_message_w(code),
        }
    }
}

// ---------------------------------------------------------------------------
// ScanFailure：扫描失败（对 UI 层的精简形态）
// ---------------------------------------------------------------------------

#[derive(Debug, Error, Clone)]
pub enum ScanFailure {
    #[error("权限不足，需要管理员")]
    AccessDenied,

    #[error("路径不存在")]
    PathNotFound,

    #[error("扫描失败: {0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// RmError：Restart Manager 调用错误
// ---------------------------------------------------------------------------

#[derive(Debug, Error, Clone)]
pub enum RmError {
    #[error("权限不足，需要管理员")]
    AccessDenied,

    #[error("路径不存在")]
    PathNotFound,

    #[error("Windows 错误 0x{code:08X}: {desc}")]
    Win32 { code: u32, desc: String },

    #[error("Restart Manager 错误: {0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// FormatMessageW 工具
// ---------------------------------------------------------------------------

/// 调用 Win32 `FormatMessageW` 取系统错误码的本地化描述字符串。
///
/// 行为细节：
/// - 使用 `FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS`，
///   让系统按当前线程语言分配缓冲区，并忽略 `%1` 等插入位（避免误用未提供的参数）。
/// - 缓冲区由 Win32 分配，使用完后必须 `LocalFree` 释放，避免泄漏。
/// - 失败兜底：当 FormatMessageW 返回 0、缓冲区指针为空、或解码出错时，返回
///   形如 `"Win32 error 0x{:08X}"` 的占位字符串，保证调用方永远拿到非空 String。
/// - 末尾 CRLF / LF 会被去除，因为 Win32 给出的描述通常带换行（不利于单行展示）。
fn format_message_w(code: u32) -> String {
    // 失败兜底文案（不可达 panic、不可分配内存、写入失败等）
    let fallback = || format!("Win32 error 0x{:08X}", code);

    // 一个指向「Win32 LocalAlloc 出来的 wide 字符串首字符」的指针。
    // FormatMessageW 在 ALLOCATE_BUFFER 模式下要求传入「指向 PWSTR 的指针，被当作 PWSTR 写回」。
    let mut buf_ptr: PWSTR = PWSTR::null();

    // SAFETY: 调用 Win32 FormatMessageW，
    // - lpBuffer 在 ALLOCATE_BUFFER 模式下被解释为 `*mut PWSTR`，
    //   故此处把 `&mut buf_ptr` 强转为 `PWSTR`（本质是 *mut u16）后传入。
    // - lpSource / Arguments 给 None / null。
    let len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_ALLOCATE_BUFFER
                | FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_IGNORE_INSERTS,
            None,
            code,
            0, // dwLanguageId = 0，系统按默认语言查找
            PWSTR(&mut buf_ptr.0 as *mut _ as *mut u16),
            0, // 在 ALLOCATE_BUFFER 模式下表示「最小字符数」，0 即可
            None,
        )
    };

    if len == 0 || buf_ptr.is_null() {
        // 即便 len == 0，buf_ptr 也可能为 null；统一兜底
        if !buf_ptr.is_null() {
            // SAFETY: Win32 已分配，需要释放避免泄漏
            unsafe {
                let _ = LocalFree(HLOCAL(buf_ptr.0 as *mut _));
            }
        }
        return fallback();
    }

    // SAFETY: buf_ptr 指向 Win32 分配的、长度为 `len`（不含末尾 NUL）的 wide 字符串。
    let slice = unsafe { std::slice::from_raw_parts(buf_ptr.0, len as usize) };
    let mut s = String::from_utf16_lossy(slice);

    // 释放 Win32 分配的缓冲
    // SAFETY: buf_ptr 由 Win32 在 ALLOCATE_BUFFER 模式下分配，配套 LocalFree 释放
    unsafe {
        let _ = LocalFree(HLOCAL(buf_ptr.0 as *mut _));
    }

    // 去除末尾的 CR / LF（Win32 描述通常以 "\r\n" 结尾）
    while matches!(s.chars().last(), Some('\r') | Some('\n') | Some(' ')) {
        s.pop();
    }

    if s.is_empty() {
        fallback()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// 单元测试（任务 2.5 中会更全面，此处仅自检关键格式）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_win32_display_contains_hex_code() {
        // 验证 Display 字符串包含 `0x{:08X}` 格式（design 要求）
        let err = AppError::Win32(5, "Access is denied.".into());
        let s = err.to_string();
        assert!(
            s.contains("0x00000005"),
            "期望包含大写零填充的错误码，实际: {s}"
        );
    }

    #[test]
    fn terminate_error_win32_display_contains_hex_code() {
        let err = TerminateError::Win32 {
            code: 0xB7,
            desc: "Cannot create a file when that file already exists.".into(),
        };
        let s = err.to_string();
        assert!(
            s.contains("0x000000B7"),
            "期望包含大写零填充的错误码，实际: {s}"
        );
    }

    #[test]
    fn app_error_from_win32_constructs_with_localized_desc() {
        // ERROR_ACCESS_DENIED == 5；FormatMessageW 应能给出非空描述
        let err = AppError::from_win32(5);
        let s = err.to_string();
        assert!(s.contains("0x00000005"));
        // 至少包含错误码后的冒号 + 一段非空描述
        assert!(
            s.len() > "Win32 错误 0x00000005: ".len(),
            "期望含本地化描述，实际: {s}"
        );
    }

    #[test]
    fn rm_error_propagates_into_app_error_via_from() {
        let rm = RmError::AccessDenied;
        let app: AppError = rm.into();
        match app {
            AppError::RestartManager(RmError::AccessDenied) => {}
            other => panic!("期望 RestartManager(AccessDenied)，实际: {other:?}"),
        }
    }

    #[test]
    fn format_message_w_unknown_code_falls_back() {
        // 选一个极不可能被系统识别的错误码；即便 FormatMessageW 失败也必须返回非空字符串
        let s = format_message_w(0xDEAD_BEEF);
        assert!(!s.is_empty(), "format_message_w 必须返回非空字符串");
    }
}
