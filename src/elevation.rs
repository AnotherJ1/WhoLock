//! 权限检测与管理员重启（Requirements 6.1, 6.3, 6.4）

use std::mem::size_of;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::error::AppError;

// ---------------------------------------------------------------------------
// RAII handle guard
// ---------------------------------------------------------------------------

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// ---------------------------------------------------------------------------
// is_elevated
// ---------------------------------------------------------------------------

/// 检测当前进程是否以管理员身份运行。
///
/// 失败时保守返回 `false`（按标准用户处理，避免误判）。
pub fn is_elevated() -> bool {
    unsafe {
        let process = GetCurrentProcess();
        let mut token = HANDLE::default();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let _guard = HandleGuard(token);

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned_size = size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            returned_size,
            &mut returned_size,
        );
        if ok.is_err() {
            return false;
        }
        elevation.TokenIsElevated != 0
    }
}

// ---------------------------------------------------------------------------
// restart_as_admin
// ---------------------------------------------------------------------------

/// 以管理员身份重启当前程序（Requirements 6.3, 6.4）。
///
/// # 返回值
/// - `Err(AppError::SelfExit)`：成功 spawn 了提权进程，调用方应执行 `exit(0)`。
/// - `Ok(())`：用户拒绝了 UAC 提示（`ERROR_CANCELLED = 1223`），静默忽略。
/// - `Err(AppError::Win32(..))` ：其他 Win32 错误。
pub fn restart_as_admin() -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;

    let exe = std::env::current_exe().map_err(|e| AppError::Internal(e.to_string()))?;

    // NUL 终止的宽字符串
    let exe_wide: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe_wide.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    let result = unsafe { ShellExecuteExW(&mut info) };

    match result {
        Ok(()) => {
            // 成功启动提权进程，通知调用方退出当前进程
            Err(AppError::SelfExit)
        }
        Err(e) => {
            let code = e.code().0 as u32;
            // ERROR_CANCELLED (1223)：用户拒绝 UAC，静默忽略
            if code == 1223 {
                Ok(())
            } else {
                Err(AppError::from_win32(code))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_elevated_returns_bool_without_panic() {
        // 只验证函数可以调用而不 panic；实际返回值取决于测试运行者的权限级别
        let result = is_elevated();
        // 在 CI（非管理员）中应为 false，在管理员终端中可能为 true
        let _ = result;
    }
}
