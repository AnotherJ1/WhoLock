//! 查询进程 token 中的用户 SID 与账户名称。

use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, TokenUser, PSID, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Threading::OpenProcessToken;

/// 查询 `process_handle` 对应进程的 token 用户 SID 及账户名。
///
/// 任意步骤失败时保守地返回 `(None, None)`，不传播错误。
pub fn query_token_user(process_handle: HANDLE) -> (Option<String>, Option<String>) {
    // 1. OpenProcessToken
    let mut token_handle = HANDLE::default();
    let ok = unsafe { OpenProcessToken(process_handle, TOKEN_QUERY, &mut token_handle) };
    if ok.is_err() {
        return (None, None);
    }

    // 确保 token 句柄被关闭
    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = HandleGuard(token_handle);

    // 2. GetTokenInformation(TokenUser) — 先获取所需缓冲区大小
    let mut needed: u32 = 0;
    unsafe {
        let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut needed);
    }
    if needed == 0 {
        return (None, None);
    }

    let mut buf: Vec<u8> = vec![0u8; needed as usize];
    let mut returned: u32 = 0;
    let ok = unsafe {
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut _),
            needed,
            &mut returned,
        )
    };
    if ok.is_err() {
        return (None, None);
    }

    // 3. 取出 SID 指针（TOKEN_USER.User.Sid）
    // SAFETY: buf 已由 GetTokenInformation 成功填充，大小足够
    let token_user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    let sid: PSID = token_user.User.Sid;

    if sid.is_invalid() {
        return (None, None);
    }

    // 4. ConvertSidToStringSidW
    let sid_string = {
        let mut sid_str_ptr: windows::core::PWSTR = windows::core::PWSTR::null();
        let ok = unsafe { ConvertSidToStringSidW(sid, &mut sid_str_ptr) };
        if ok.is_err() || sid_str_ptr.is_null() {
            None
        } else {
            // 计算宽字符串长度
            let len = unsafe {
                let mut p = sid_str_ptr.0;
                let mut count = 0usize;
                while *p != 0 {
                    p = p.add(1);
                    count += 1;
                }
                count
            };
            let slice = unsafe { std::slice::from_raw_parts(sid_str_ptr.0, len) };
            let s = String::from_utf16_lossy(slice).to_owned();
            unsafe {
                let _ = LocalFree(HLOCAL(sid_str_ptr.0 as *mut _));
            }
            Some(s)
        }
    };

    // 5. LookupAccountSidW — 先获取所需缓冲区大小
    let account_string = {
        let mut name_len: u32 = 0;
        let mut domain_len: u32 = 0;
        let mut sid_name_use = windows::Win32::Security::SID_NAME_USE::default();

        // 第一次调用获取所需大小（预期失败 ERROR_INSUFFICIENT_BUFFER）
        unsafe {
            let _ = LookupAccountSidW(
                None,
                sid,
                windows::core::PWSTR::null(),
                &mut name_len,
                windows::core::PWSTR::null(),
                &mut domain_len,
                &mut sid_name_use,
            );
        }

        if name_len == 0 {
            None
        } else {
            let mut name_buf: Vec<u16> = vec![0u16; name_len as usize];
            let mut domain_buf: Vec<u16> = vec![0u16; domain_len.max(1) as usize];
            let ok = unsafe {
                LookupAccountSidW(
                    None,
                    sid,
                    windows::core::PWSTR(name_buf.as_mut_ptr()),
                    &mut name_len,
                    windows::core::PWSTR(domain_buf.as_mut_ptr()),
                    &mut domain_len,
                    &mut sid_name_use,
                )
            };
            if ok.is_err() {
                None
            } else {
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                if domain_len > 0 {
                    let domain = String::from_utf16_lossy(&domain_buf[..domain_len as usize]);
                    Some(format!("{}\\{}", domain, name))
                } else {
                    Some(name.to_owned())
                }
            }
        }
    };

    (sid_string, account_string)
}
