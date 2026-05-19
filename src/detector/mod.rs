//! Lock_Detector：Restart Manager 调用封装与 ProcessRecord 装配模块。
//!
//! 本模块定义核心数据类型 `ProcessRecord` 与 `AppType`，
//! 以及子模块 `enumerate`（文件夹直接子项枚举）和 `restart_manager`（RM API 抽象）。

use std::path::PathBuf;

pub mod enumerate;
pub mod restart_manager;
pub mod win32_rm;

pub use enumerate::enumerate_direct_children;
pub use restart_manager::{RestartManagerApi, RmProcessInfo};
pub use win32_rm::Win32RestartManager;

use crate::error::{RmError, ScanFailure};
use crate::process_info;
use crate::state::target::{TargetItem, TargetKind};
use crate::sys_classifier::{is_system_process, PartialRecord};

/// 进程的应用类型，对应 Win32 `RM_APP_TYPE` 语义，附加内部分类。
#[derive(Clone, Debug)]
pub enum AppType {
    /// 普通桌面应用程序（有窗口）。
    Application,
    /// Windows 服务（无交互界面）。
    Service,
    /// 控制台程序。
    Console,
    /// 系统关键进程（如 lsass、csrss），不可强制结束。
    Critical,
    /// 无法识别的类型。
    Unknown,
}

/// 正在锁定目标路径的单个进程的完整描述。
///
/// 由 `detector::scan`（Wave 5A）装配并通过通道发送给 UI 层。
#[derive(Clone, Debug)]
pub struct ProcessRecord {
    /// 进程 PID。
    pub pid: u32,
    /// 进程名称（不含路径，如 `explorer.exe`）。
    pub name: String,
    /// 进程可执行文件的完整路径（查询失败时为 `None`）。
    pub image_path: Option<PathBuf>,
    /// 在 Target_Item 的路径集合中，该进程锁定的具体子路径（仅文件夹模式下有意义）。
    pub locked_subpath: Option<PathBuf>,
    /// 该进程锁定的子项数量（文件夹模式下的汇总计数）。
    pub locked_subitem_count: u32,
    /// 进程启动时间（Windows FILETIME 格式，100 纳秒为单位，自 1601-01-01 起计）。
    /// 用于 PID 复用检测；查询失败时为 `None`。
    pub start_time: Option<i64>,
    /// 应用类型。
    pub app_type: AppType,
    /// 是否为系统进程（PID 0/4 或命中黑名单）。
    pub is_system: bool,
    /// 进程所有者的 SID 字符串（如 `S-1-5-18`），查询失败时为 `None`。
    pub user_sid: Option<String>,
    /// 进程所有者的账号名（如 `SYSTEM`、`DOMAIN\user`），查询失败时为 `None`。
    pub user_account: Option<String>,
}

// ---------------------------------------------------------------------------
// scan：对单个 TargetItem 执行一次占用检测
// ---------------------------------------------------------------------------

/// 对单个 TargetItem 执行一次占用检测，返回 ProcessRecord 列表。
pub fn scan(
    target: &TargetItem,
    rm: &dyn RestartManagerApi,
) -> Result<Vec<ProcessRecord>, ScanFailure> {
    // 1. 根据 TargetKind 决定注册路径
    let paths: Vec<PathBuf> = match target.kind {
        TargetKind::Directory => enumerate_direct_children(&target.path),
        TargetKind::File => vec![target.path.clone()],
    };

    // 2. 调用 Restart Manager
    let rm_results = rm.scan(&paths).map_err(|e| match e {
        RmError::AccessDenied => ScanFailure::AccessDenied,
        RmError::PathNotFound => ScanFailure::PathNotFound,
        other => ScanFailure::Other(other.to_string()),
    })?;

    // 3. 对每条 RmProcessInfo 查询进程详细信息，构建 RawRecord
    let is_dir = matches!(target.kind, TargetKind::Directory);
    let raw: Vec<RawRecord> = rm_results
        .into_iter()
        .map(|rm_info| {
            let snapshot = process_info::snapshot(rm_info.pid).ok();
            let image_path = snapshot.as_ref().and_then(|s| s.image_path.clone());
            let user_sid = snapshot.as_ref().and_then(|s| s.user_sid.clone());
            let user_account = snapshot.as_ref().and_then(|s| s.user_account.clone());

            let partial = PartialRecord {
                pid: rm_info.pid,
                name: rm_info.app_name.clone(),
                image_path: image_path.clone(),
                user_sid: user_sid.clone(),
            };
            let is_system = is_system_process(&partial);
            let app_type = map_app_type(rm_info.app_type);

            RawRecord {
                pid: rm_info.pid,
                name: rm_info.app_name,
                image_path,
                start_time: rm_info
                    .start_time
                    .map(|ft| ((ft.dwHighDateTime as i64) << 32) | (ft.dwLowDateTime as i64)),
                app_type,
                is_system,
                user_sid,
                user_account,
                // 记录该进程命中的具体路径（用于文件夹模式）
                matched_path: paths.first().cloned(),
            }
        })
        .collect();

    // 4. 合并同 PID 的多条记录
    Ok(merge_process_records(raw, &paths, is_dir))
}

// ---------------------------------------------------------------------------
// RawRecord：合并前的内部原始记录
// ---------------------------------------------------------------------------

/// 内部原始记录（合并前）
#[derive(Debug)]
pub struct RawRecord {
    pub pid: u32,
    pub name: String,
    pub image_path: Option<PathBuf>,
    pub start_time: Option<i64>,
    pub app_type: AppType,
    pub is_system: bool,
    pub user_sid: Option<String>,
    pub user_account: Option<String>,
    pub matched_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// merge_process_records：将同 PID 的多条 RawRecord 合并为单条 ProcessRecord
// ---------------------------------------------------------------------------

/// 将同一 PID 的多条 RawRecord 合并为单条 ProcessRecord。
///
/// - `locked_subitem_count` = 同 PID 出现次数
/// - `locked_subpath` = 文件夹模式下取第一个匹配路径
/// - 输出按 pid 排序，确保稳定
pub fn merge_process_records(
    raw: Vec<RawRecord>,
    registered_paths: &[PathBuf],
    is_dir: bool,
) -> Vec<ProcessRecord> {
    use std::collections::BTreeMap;

    // BTreeMap 自动按 pid 升序排序
    let mut map: BTreeMap<u32, (RawRecord, u32, Option<PathBuf>)> = BTreeMap::new();

    for rec in raw {
        let matched = if is_dir {
            rec.matched_path
                .clone()
                .filter(|p| registered_paths.contains(p))
        } else {
            None
        };

        map.entry(rec.pid)
            .and_modify(|(_, count, subpath)| {
                *count += 1;
                // 只保留第一个匹配路径
                if subpath.is_none() {
                    *subpath = matched.clone();
                }
            })
            .or_insert((rec, 1, matched));
    }

    map.into_values()
        .map(|(rec, count, subpath)| ProcessRecord {
            pid: rec.pid,
            name: rec.name,
            image_path: rec.image_path,
            locked_subpath: subpath,
            locked_subitem_count: count,
            start_time: rec.start_time,
            app_type: rec.app_type,
            is_system: rec.is_system,
            user_sid: rec.user_sid,
            user_account: rec.user_account,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// map_app_type：RM_APP_TYPE → AppType
// ---------------------------------------------------------------------------

/// 将 Win32 `RM_APP_TYPE` 原始值映射到 `AppType`。
///
/// SDK 定义值：
/// - `RmUnknownApp` = 0
/// - `RmMainWindow` = 1
/// - `RmOtherWindow` = 2
/// - `RmService` = 3
/// - `RmExplorer` = 4
/// - `RmConsole` = 5
/// - `RmCritical` = 1000
fn map_app_type(rm_type: u32) -> AppType {
    match rm_type {
        1 | 2 | 4 => AppType::Application,
        3 => AppType::Service,
        5 => AppType::Console,
        1000 => AppType::Critical,
        _ => AppType::Unknown,
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_deduplicates_by_pid() {
        let raw = vec![
            RawRecord {
                pid: 100,
                name: "foo.exe".into(),
                image_path: None,
                start_time: None,
                app_type: AppType::Application,
                is_system: false,
                user_sid: None,
                user_account: None,
                matched_path: None,
            },
            RawRecord {
                pid: 100,
                name: "foo.exe".into(),
                image_path: None,
                start_time: None,
                app_type: AppType::Application,
                is_system: false,
                user_sid: None,
                user_account: None,
                matched_path: None,
            },
            RawRecord {
                pid: 200,
                name: "bar.exe".into(),
                image_path: None,
                start_time: None,
                app_type: AppType::Service,
                is_system: false,
                user_sid: None,
                user_account: None,
                matched_path: None,
            },
        ];
        let result = merge_process_records(raw, &[], false);
        assert_eq!(result.len(), 2);
        let pid100 = result.iter().find(|r| r.pid == 100).unwrap();
        assert_eq!(pid100.locked_subitem_count, 2);
        let pid200 = result.iter().find(|r| r.pid == 200).unwrap();
        assert_eq!(pid200.locked_subitem_count, 1);
        // 按 pid 排序
        assert_eq!(result[0].pid, 100);
        assert_eq!(result[1].pid, 200);
    }

    #[test]
    fn merge_subitem_count_sums_correctly() {
        let raw: Vec<RawRecord> = (0..5)
            .map(|_| RawRecord {
                pid: 42,
                name: "multi.exe".into(),
                image_path: None,
                start_time: None,
                app_type: AppType::Unknown,
                is_system: false,
                user_sid: None,
                user_account: None,
                matched_path: None,
            })
            .collect();
        let result = merge_process_records(raw, &[], false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].locked_subitem_count, 5);
    }

    // ---------------------------------------------------------------------
    // Feature: file-lock-inspector, Property 3: ProcessRecord 合并不变量
    // Validates: Requirements 2.2, 2.3, 2.4
    // ---------------------------------------------------------------------
    use proptest::prelude::*;

    /// 生成单条 RawRecord（pid 在 1..=20 之间，便于产生重复）
    fn arb_raw_record() -> impl Strategy<Value = RawRecord> {
        (
            1u32..=20,
            "[a-z]{3,8}\\.exe",
            proptest::option::of(0i32..100i32),
        )
            .prop_map(|(pid, name, idx)| {
                let matched = idx.map(|i| std::path::PathBuf::from(format!("C:/p/{}.txt", i)));
                RawRecord {
                    pid,
                    name,
                    image_path: None,
                    start_time: None,
                    app_type: AppType::Unknown,
                    is_system: false,
                    user_sid: None,
                    user_account: None,
                    matched_path: matched,
                }
            })
    }

    proptest! {
        /// (a) 输出 PID 互不相同；
        /// (b) Σ locked_subitem_count == 输入条目数；
        /// (c) 文件夹模式下每个 locked_subpath 必须 ∈ registered_paths
        #[test]
        fn prop_merge_invariants(raw in proptest::collection::vec(arb_raw_record(), 0..30)) {
            // 收集所有可能 matched_path 形成 registered_paths（保证 c 条件可被满足）
            let registered: Vec<std::path::PathBuf> = raw.iter()
                .filter_map(|r| r.matched_path.clone())
                .collect();
            let total_input = raw.len() as u32;

            // 分别测试 file 模式和 dir 模式
            for is_dir in [false, true] {
                let merged = merge_process_records(
                    raw.iter().map(|r| RawRecord {
                        pid: r.pid,
                        name: r.name.clone(),
                        image_path: r.image_path.clone(),
                        start_time: r.start_time,
                        app_type: r.app_type.clone(),
                        is_system: r.is_system,
                        user_sid: r.user_sid.clone(),
                        user_account: r.user_account.clone(),
                        matched_path: r.matched_path.clone(),
                    }).collect(),
                    &registered,
                    is_dir,
                );

                // (a) PID 互不相同
                let mut pids: Vec<u32> = merged.iter().map(|r| r.pid).collect();
                pids.sort();
                pids.dedup();
                prop_assert_eq!(pids.len(), merged.len());

                // (b) Σ locked_subitem_count == 输入条目数
                let sum: u32 = merged.iter().map(|r| r.locked_subitem_count).sum();
                prop_assert_eq!(sum, total_input);

                // (c) 文件夹模式下：locked_subpath 若 Some 则必须 ∈ registered
                if is_dir {
                    for r in &merged {
                        if let Some(sp) = &r.locked_subpath {
                            prop_assert!(registered.contains(sp),
                                "locked_subpath {:?} should be in registered_paths", sp);
                        }
                    }
                } else {
                    // 文件模式下 locked_subpath 必须 None
                    for r in &merged {
                        prop_assert!(r.locked_subpath.is_none());
                    }
                }
            }
        }
    }
}
