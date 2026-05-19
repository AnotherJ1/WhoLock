//! 系统进程三层判定算法（design.md System_Process_Classifier 节）
pub mod blacklist;
use blacklist::is_blacklisted;
use std::path::Path;

/// is_system_process 的输入记录（纯数据，不依赖 Win32）
#[derive(Debug, Clone)]
pub struct PartialRecord {
    pub pid: u32,
    pub name: String,
    pub image_path: Option<std::path::PathBuf>,
    pub user_sid: Option<String>,
}

/// 三层判定：
/// Layer 1: PID 0/4 或名字在黑名单
/// Layer 2: 系统 SID ∩ System32/SysWOW64 路径
/// Layer 3: image_path 与 user_sid 同时为 None（保守）
pub fn is_system_process(rec: &PartialRecord) -> bool {
    // Layer 1
    if rec.pid == 0 || rec.pid == 4 {
        return true;
    }
    if is_blacklisted(&rec.name) {
        return true;
    }

    // Layer 2
    let well_known_sids = ["S-1-5-18", "S-1-5-19", "S-1-5-20"];
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let sys32 = Path::new(&windir).join("System32");
    let syswow = Path::new(&windir).join("SysWOW64");
    let in_sys_dir = rec
        .image_path
        .as_ref()
        .map(|p| p.starts_with(&sys32) || p.starts_with(&syswow))
        .unwrap_or(false);
    let sys_account = rec
        .user_sid
        .as_deref()
        .map(|s| well_known_sids.contains(&s))
        .unwrap_or(false);
    if in_sys_dir && sys_account {
        return true;
    }

    // Layer 3: 信息缺失保守判定
    if rec.image_path.is_none() && rec.user_sid.is_none() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_record(
        pid: u32,
        name: &str,
        image_path: Option<&str>,
        user_sid: Option<&str>,
    ) -> PartialRecord {
        PartialRecord {
            pid,
            name: name.to_string(),
            image_path: image_path.map(PathBuf::from),
            user_sid: user_sid.map(str::to_string),
        }
    }

    // --- Layer 1 tests ---

    #[test]
    fn layer1_pid_zero_is_system() {
        let rec = make_record(
            0,
            "Idle",
            Some("C:\\Windows\\System32\\ntoskrnl.exe"),
            Some("S-1-5-32-544"),
        );
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer1_pid_four_is_system() {
        let rec = make_record(
            4,
            "System",
            Some("C:\\Windows\\System32\\ntoskrnl.exe"),
            Some("S-1-5-32-544"),
        );
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer1_blacklisted_name_is_system() {
        let rec = make_record(
            1234,
            "svchost.exe",
            Some("C:\\Users\\user\\svchost.exe"),
            Some("S-1-5-21-999"),
        );
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer1_blacklisted_name_case_insensitive() {
        let rec = make_record(
            1234,
            "LSASS.EXE",
            Some("C:\\Users\\user\\lsass.exe"),
            Some("S-1-5-21-999"),
        );
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer1_registry_pseudo_process_is_system() {
        let rec = make_record(88, "Registry", None, None);
        // Would also hit Layer 1 (blacklist) before Layer 3
        assert!(is_system_process(&rec));
    }

    // --- Layer 2 tests ---

    #[test]
    fn layer2_system32_with_system_sid_is_system() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let path = format!("{}\\System32\\somesys.exe", windir);
        let rec = make_record(9999, "somesys.exe", Some(&path), Some("S-1-5-18"));
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer2_syswow64_with_local_service_sid_is_system() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let path = format!("{}\\SysWOW64\\wow64.exe", windir);
        let rec = make_record(9999, "wow64.exe", Some(&path), Some("S-1-5-19"));
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer2_system32_path_but_user_sid_not_system() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let path = format!("{}\\System32\\notepad.exe", windir);
        // User SID — not a well-known system SID
        let rec = make_record(9999, "notepad.exe", Some(&path), Some("S-1-5-21-1234"));
        assert!(!is_system_process(&rec));
    }

    #[test]
    fn layer2_system_sid_but_non_system_path_not_system() {
        let rec = make_record(
            9999,
            "mytool.exe",
            Some("C:\\Users\\user\\mytool.exe"),
            Some("S-1-5-18"),
        );
        assert!(!is_system_process(&rec));
    }

    // --- Layer 3 tests ---

    #[test]
    fn layer3_both_none_is_system() {
        // pid != 0/4, not blacklisted, no image_path, no user_sid
        let rec = make_record(5000, "unknown.exe", None, None);
        assert!(is_system_process(&rec));
    }

    #[test]
    fn layer3_only_image_path_none_not_system_if_has_user_sid() {
        // Has user SID but no path — Layer 3 requires BOTH to be None
        let rec = make_record(5000, "unknown.exe", None, Some("S-1-5-21-1234"));
        assert!(!is_system_process(&rec));
    }

    #[test]
    fn layer3_only_user_sid_none_not_system_if_has_image_path() {
        // Has path but no user SID — Layer 3 requires BOTH to be None
        let rec = make_record(
            5000,
            "unknown.exe",
            Some("C:\\Program Files\\app.exe"),
            None,
        );
        assert!(!is_system_process(&rec));
    }

    // --- Normal user process tests ---

    #[test]
    fn normal_user_process_not_system() {
        let rec = make_record(
            1234,
            "notepad.exe",
            Some("C:\\Windows\\notepad.exe"),
            Some("S-1-5-21-1234-5678"),
        );
        assert!(!is_system_process(&rec));
    }

    #[test]
    fn browser_process_not_system() {
        let rec = make_record(
            8888,
            "chrome.exe",
            Some("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            Some("S-1-5-21-987654321"),
        );
        assert!(!is_system_process(&rec));
    }
}

// ---------------------------------------------------------------------------
// Property-Based Tests — Property 8: 系统进程判定算法
// Requirements: 5.1, 5.2
// ---------------------------------------------------------------------------

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// PID 0 和 PID 4 对任意进程名始终返回 true。
        #[test]
        fn prop_system_process_pid_0_and_4_always_true(
            name in "[a-z]{1,20}\\.exe"
        ) {
            let rec0 = PartialRecord {
                pid: 0,
                name: name.clone(),
                image_path: None,
                user_sid: None,
            };
            let rec4 = PartialRecord {
                pid: 4,
                name,
                image_path: None,
                user_sid: None,
            };
            prop_assert!(is_system_process(&rec0));
            prop_assert!(is_system_process(&rec4));
        }

        /// 黑名单中的进程名，无论 PID 为何值，始终返回 true。
        #[test]
        fn prop_blacklisted_names_always_system(
            pid in 100u32..60000u32
        ) {
            for name in &["svchost.exe", "csrss.exe", "lsass.exe", "wininit.exe"] {
                let rec = PartialRecord {
                    pid,
                    name: name.to_string(),
                    image_path: Some(std::path::PathBuf::from("C:\\foo\\bar.exe")),
                    user_sid: Some("S-1-5-21-123".to_string()),
                };
                prop_assert!(
                    is_system_process(&rec),
                    "expected system for pid={} name={}", pid, name
                );
            }
        }

        /// image_path 与 user_sid 同时为 None → 保守判定为系统进程。
        #[test]
        fn prop_missing_both_fields_is_conservative(
            pid in 100u32..60000u32,
            name in "[a-z]{5,15}"
        ) {
            let rec = PartialRecord {
                pid,
                name,
                image_path: None,
                user_sid: None,
            };
            prop_assert!(is_system_process(&rec));
        }

        /// 普通用户进程（非系统路径、非系统 SID、非黑名单名称）→ 不是系统进程。
        #[test]
        fn prop_normal_user_process_not_system(
            pid in 100u32..60000u32
        ) {
            let rec = PartialRecord {
                pid,
                name: "myapp.exe".to_string(),
                image_path: Some(std::path::PathBuf::from(
                    "C:\\Users\\user\\myapp.exe",
                )),
                user_sid: Some("S-1-5-21-999999-888888-777777-1001".to_string()),
            };
            prop_assert!(!is_system_process(&rec));
        }
    }
}
