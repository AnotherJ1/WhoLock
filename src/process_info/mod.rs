//! process_info：查询进程映像路径与 token 用户信息。

pub mod token;

use std::path::PathBuf;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::error::AppError;

/// 进程快照：在一次 OpenProcess 中尽量收集所有可获取的信息。
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    /// 进程完整映像路径（可执行文件的完整路径）。`None` 表示无法获取。
    pub image_path: Option<PathBuf>,
    /// 用户 SID 字符串，形如 `"S-1-5-18"`。`None` 表示无法获取。
    pub user_sid: Option<String>,
    /// 域\账户名称字符串，形如 `"NT AUTHORITY\\SYSTEM"`。`None` 表示无法获取。
    pub user_account: Option<String>,
}

/// 查询指定 PID 的进程快照。
///
/// 任意子步骤失败时不传播错误，而是将对应字段置 `None`，保证调用方
/// 总能拿到一个（可能部分填充的）快照。
///
/// 仅当 `OpenProcess` 本身失败（进程不存在或无任何权限）时返回 `Err`。
pub fn snapshot(pid: u32) -> Result<ProcessSnapshot, AppError> {
    // 1. 打开进程句柄（仅请求最低权限）
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let handle = match handle {
        Ok(h) => h,
        Err(e) => {
            let code = e.code().0 as u32;
            return Err(AppError::from_win32(code));
        }
    };

    // 确保句柄在本函数退出时被关闭
    // 使用 scopeguard 风格：直接在结尾 CloseHandle
    // （windows-rs HANDLE 不实现 Drop，需手动关闭）

    // 2. 查询完整映像路径（QueryFullProcessImageNameW）
    let image_path = {
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
        };
        if ok.is_ok() && size > 0 {
            let path_str: String = String::from_utf16_lossy(&buf[..size as usize]).to_owned();
            Some(PathBuf::from(path_str))
        } else {
            None
        }
    };

    // 3. 查询 token 用户
    let (user_sid, user_account) = token::query_token_user(handle);

    // 4. 关闭句柄
    unsafe {
        let _ = CloseHandle(handle);
    }

    Ok(ProcessSnapshot {
        image_path,
        user_sid,
        user_account,
    })
}
