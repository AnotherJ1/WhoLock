//! 系统进程硬黑名单（Layer 1 of `is_system_process`）。
//!
//! 该列表来自需求 5.1 与设计文档 "System_Process_Classifier" 节，
//! 列出 Windows 内核 / 会话 / 安全子系统的关键进程。
//! 这些进程一旦被强制结束，会导致系统不稳定甚至蓝屏，
//! 因此本应用始终拒绝对其执行 Force_Terminate_Action。
//!
//! 名字比较使用 ASCII 大小写不敏感（Win32 文件系统语义），
//! 通过 [`str::eq_ignore_ascii_case`] 实现，避免在比较时分配新字符串。
//!
//! 关于 lsm.exe：在 Win10/11 上 lsm 已并入 services.exe，
//! 仍保留在列表中作为防御性匹配（需求 5.1 显式约定）。

/// 硬编码的系统进程名黑名单。
///
/// 列表完整性约束（需求 5.1）：必须包含且仅包含这 10 项，
/// 与 design.md "System_Process_Classifier" 节保持一致。
pub const BLACKLIST: &[&str] = &[
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsm.exe",
    "svchost.exe",
    "Registry",
    "MemCompression",
];

/// 判断给定进程名是否命中硬黑名单。
///
/// 比较语义为 ASCII 大小写不敏感，例如 `"SVCHOST.EXE"` 与 `"svchost.exe"`
/// 视为同名。Restart Manager 返回的 `AppName` 字段在不同 Windows 版本上
/// 大小写存在差异，本函数据此对调用方屏蔽该差异。
#[inline]
pub fn is_blacklisted(name: &str) -> bool {
    BLACKLIST.iter().any(|entry| entry.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist_has_exactly_ten_entries() {
        // 需求 5.1：黑名单长度必须为 10
        assert_eq!(BLACKLIST.len(), 10);
    }

    #[test]
    fn svchost_exact_case_is_blacklisted() {
        assert!(is_blacklisted("svchost.exe"));
    }

    #[test]
    fn svchost_upper_case_is_blacklisted() {
        // 大小写不敏感比较
        assert!(is_blacklisted("SVCHOST.EXE"));
    }

    #[test]
    fn notepad_is_not_blacklisted() {
        assert!(!is_blacklisted("notepad.exe"));
    }

    #[test]
    fn registry_pseudo_process_is_blacklisted() {
        // Registry / MemCompression 没有 .exe 后缀
        assert!(is_blacklisted("Registry"));
        assert!(is_blacklisted("registry"));
        assert!(is_blacklisted("MemCompression"));
    }

    #[test]
    fn empty_name_is_not_blacklisted() {
        assert!(!is_blacklisted(""));
    }
}
