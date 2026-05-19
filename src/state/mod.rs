//! 应用状态层（AppState、TargetItem、UiCmd 等）模块。
//!
//! 子模块组织：
//! - `target`：`TargetId` / `TargetKind` / `TargetStatus` / `TargetItem`
//! - `app_state`：`AppState` / `PrivilegeLevel` / `UiToast`

pub mod app_state;
pub mod target;

pub use app_state::{AppState, PrivilegeLevel, UiToast};

use crate::state::target::{TargetId, TargetItem, TargetKind, TargetStatus};
use std::path::PathBuf;

/// 文件系统探针 trait（便于 PBT 注入虚拟文件系统）
pub trait FsProbe: Send + Sync {
    fn exists(&self, path: &std::path::Path) -> bool;
    fn is_dir(&self, path: &std::path::Path) -> bool;
}

/// 真实文件系统实现
pub struct RealFsProbe;
impl FsProbe for RealFsProbe {
    fn exists(&self, path: &std::path::Path) -> bool {
        path.exists()
    }
    fn is_dir(&self, path: &std::path::Path) -> bool {
        path.is_dir()
    }
}

#[derive(Debug)]
pub enum AddError {
    PathNotFound,
    Duplicate,
}

/// UI 层命令枚举
pub enum UiCmd {
    AddPaths(Vec<PathBuf>),
    RemoveTarget(TargetId),
    ClearAll {
        confirmed: bool,
    },
    SetInterval(u32),
    SetPrivilege(PrivilegeLevel),
    DismissToast,
    ShowToast(UiToast),
    /// 请求打开"强制结束"二次确认对话框（由 process_row 点击触发）
    OpenTerminateDialog {
        pid: u32,
        process_name: String,
        target_id: TargetId,
        start_time: Option<i64>,
    },
    /// 用户在确认对话框中点击"确认结束"，由 app.rs 调用 force_terminate
    ConfirmTerminate {
        pid: u32,
        process_name: String,
        target_id: TargetId,
        start_time: Option<i64>,
    },
}

pub fn try_add_target(
    state: &mut AppState,
    path: PathBuf,
    fs: &dyn FsProbe,
) -> Result<TargetId, AddError> {
    if !fs.exists(&path) {
        return Err(AddError::PathNotFound);
    }
    if state.targets.values().any(|t| t.path == path) {
        return Err(AddError::Duplicate);
    }
    let id = state.next_target_id();
    let kind = if fs.is_dir(&path) {
        TargetKind::Directory
    } else {
        TargetKind::File
    };
    state.targets.insert(
        id,
        TargetItem {
            id,
            path,
            kind,
            status: TargetStatus::Pending,
            processes: Vec::new(),
            last_scanned_at: None,
        },
    );
    Ok(id)
}

pub struct BatchAddSummary {
    pub added: usize,
    pub skipped_existing: usize,
    pub rejected_missing: usize,
}

impl BatchAddSummary {
    pub fn to_toast_message(&self) -> String {
        format!(
            "成功添加 {} 项 / 跳过 {} 项已存在 / 拒绝 {} 项不存在",
            self.added, self.skipped_existing, self.rejected_missing
        )
    }
}

pub fn try_add_targets(
    state: &mut AppState,
    paths: Vec<PathBuf>,
    fs: &dyn FsProbe,
) -> BatchAddSummary {
    let mut summary = BatchAddSummary {
        added: 0,
        skipped_existing: 0,
        rejected_missing: 0,
    };
    for path in paths {
        match try_add_target(state, path, fs) {
            Ok(_) => summary.added += 1,
            Err(AddError::Duplicate) => summary.skipped_existing += 1,
            Err(AddError::PathNotFound) => summary.rejected_missing += 1,
        }
    }
    summary
}

use crate::monitor::ScanEvent;

/// 将 `ScanEvent` 应用到 `AppState`，更新目标的状态与进程列表。
pub fn apply_scan_event(state: &mut AppState, ev: ScanEvent) {
    match ev {
        ScanEvent::Started(id) => {
            if let Some(t) = state.targets.get_mut(&id) {
                t.status = TargetStatus::Scanning;
            }
        }
        ScanEvent::Completed(id, records) => {
            if let Some(t) = state.targets.get_mut(&id) {
                let count = records.len();
                t.processes = records;
                t.status = if count == 0 {
                    TargetStatus::Idle
                } else {
                    TargetStatus::Locked { count }
                };
                t.last_scanned_at = Some(std::time::Instant::now());
            }
        }
        ScanEvent::Failed(id, failure) => {
            if let Some(t) = state.targets.get_mut(&id) {
                t.status = match &failure {
                    crate::error::ScanFailure::AccessDenied => TargetStatus::AccessDenied,
                    _ => TargetStatus::Failed {
                        reason: failure.to_string(),
                    },
                };
            }
        }
    }
}

/// 将 `UiCmd` 应用到 `AppState`，执行相应的状态变更。
pub fn apply(state: &mut AppState, cmd: UiCmd, fs: &dyn FsProbe) {
    match cmd {
        UiCmd::AddPaths(paths) => {
            try_add_targets(state, paths, fs);
        }
        UiCmd::RemoveTarget(id) => {
            state.targets.remove(&id);
        }
        UiCmd::ClearAll { confirmed: true } => {
            state.targets.clear();
        }
        UiCmd::ClearAll { confirmed: false } => {}
        UiCmd::SetInterval(ms) => {
            state.polling_interval_ms = ms;
        }
        UiCmd::SetPrivilege(p) => {
            state.privilege = p;
        }
        UiCmd::DismissToast => {
            state.last_error = None;
        }
        UiCmd::ShowToast(t) => {
            state.last_error = Some(t);
        }
        // 这两条 UI 命令不修改 AppState，由 app.rs 派发到对话框/终止流程
        UiCmd::OpenTerminateDialog { .. } => {}
        UiCmd::ConfirmTerminate { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    /// Mock FsProbe: paths in `existing` exist; paths in `dirs` are directories.
    struct MockFs {
        existing: HashSet<PathBuf>,
        dirs: HashSet<PathBuf>,
    }

    impl MockFs {
        fn new(existing: &[&str], dirs: &[&str]) -> Self {
            Self {
                existing: existing.iter().map(PathBuf::from).collect(),
                dirs: dirs.iter().map(PathBuf::from).collect(),
            }
        }
    }

    impl FsProbe for MockFs {
        fn exists(&self, path: &Path) -> bool {
            self.existing.contains(path)
        }
        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.contains(path)
        }
    }

    #[test]
    fn try_add_target_ok_file() {
        let mut state = AppState::new_default();
        let fs = MockFs::new(&["/tmp/foo.txt"], &[]);
        let result = try_add_target(&mut state, PathBuf::from("/tmp/foo.txt"), &fs);
        assert!(result.is_ok());
        let id = result.unwrap();
        let item = state.targets.get(&id).unwrap();
        assert_eq!(item.kind, TargetKind::File);
        assert!(matches!(item.status, TargetStatus::Pending));
    }

    #[test]
    fn try_add_target_ok_directory() {
        let mut state = AppState::new_default();
        let fs = MockFs::new(&["/tmp/mydir"], &["/tmp/mydir"]);
        let result = try_add_target(&mut state, PathBuf::from("/tmp/mydir"), &fs);
        assert!(result.is_ok());
        let id = result.unwrap();
        let item = state.targets.get(&id).unwrap();
        assert_eq!(item.kind, TargetKind::Directory);
    }

    #[test]
    fn try_add_target_err_path_not_found() {
        let mut state = AppState::new_default();
        let fs = MockFs::new(&[], &[]);
        let result = try_add_target(&mut state, PathBuf::from("/nonexistent/path.txt"), &fs);
        assert!(matches!(result, Err(AddError::PathNotFound)));
        assert!(state.targets.is_empty());
    }

    #[test]
    fn try_add_target_err_duplicate() {
        let mut state = AppState::new_default();
        let fs = MockFs::new(&["/tmp/foo.txt"], &[]);
        let _ = try_add_target(&mut state, PathBuf::from("/tmp/foo.txt"), &fs);
        let result = try_add_target(&mut state, PathBuf::from("/tmp/foo.txt"), &fs);
        assert!(matches!(result, Err(AddError::Duplicate)));
        assert_eq!(state.targets.len(), 1);
    }

    #[test]
    fn try_add_targets_batch_summary() {
        let mut state = AppState::new_default();
        let fs = MockFs::new(&["/a.txt", "/b.txt"], &[]);
        // Pre-add /a.txt so it's a duplicate in the batch
        let _ = try_add_target(&mut state, PathBuf::from("/a.txt"), &fs);

        let paths = vec![
            PathBuf::from("/a.txt"),       // duplicate
            PathBuf::from("/b.txt"),       // new file
            PathBuf::from("/missing.txt"), // not found
        ];
        let summary = try_add_targets(&mut state, paths, &fs);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.skipped_existing, 1);
        assert_eq!(summary.rejected_missing, 1);
    }

    #[test]
    fn batch_add_summary_toast_message_format() {
        let summary = BatchAddSummary {
            added: 3,
            skipped_existing: 2,
            rejected_missing: 1,
        };
        let msg = summary.to_toast_message();
        assert!(msg.contains("3"), "should contain added count");
        assert!(msg.contains("2"), "should contain skipped count");
        assert!(msg.contains("1"), "should contain rejected count");
        assert_eq!(msg, "成功添加 3 项 / 跳过 2 项已存在 / 拒绝 1 项不存在");
    }

    #[test]
    fn batch_add_summary_all_zeros_message() {
        let summary = BatchAddSummary {
            added: 0,
            skipped_existing: 0,
            rejected_missing: 0,
        };
        let msg = summary.to_toast_message();
        assert_eq!(msg, "成功添加 0 项 / 跳过 0 项已存在 / 拒绝 0 项不存在");
    }

    // -----------------------------------------------------------------------
    // PBT: Property 1, 5, 10, 12, 13
    // -----------------------------------------------------------------------
    use proptest::prelude::*;

    /// 生成路径：从一个固定小集合中选取，便于产生重复
    fn arb_path() -> impl Strategy<Value = PathBuf> {
        (0u32..15u32).prop_map(|i| PathBuf::from(format!("C:/data/p{}.txt", i)))
    }

    /// 用 HashSet 构造 MockFs（便于 PBT 注入）
    fn mock_fs_from_set(existing: &HashSet<PathBuf>) -> MockFs {
        MockFs {
            existing: existing.clone(),
            dirs: HashSet::new(),
        }
    }

    proptest! {
        // Feature: file-lock-inspector, Property 1: try_add_target 的输入归并语义
        // Validates: Requirements 1.3, 1.4, 1.5, 1.6
        #[test]
        fn prop_try_add_target_dedup_and_existence(
            inputs in proptest::collection::vec(arb_path(), 0..20),
            existing_set in proptest::collection::vec(arb_path(), 0..15),
        ) {
            let existing: HashSet<PathBuf> = existing_set.into_iter().collect();
            let fs = MockFs { existing: existing.clone(), dirs: HashSet::new() };
            let mut state = AppState::new_default();

            for p in &inputs {
                let _ = try_add_target(&mut state, p.clone(), &fs);
            }

            // 期望集合 = dedup(inputs.filter(exists))
            let expected: HashSet<PathBuf> = inputs.iter()
                .filter(|p| existing.contains(*p))
                .cloned()
                .collect();
            let actual: HashSet<PathBuf> = state.targets.values()
                .map(|t| t.path.clone())
                .collect();
            prop_assert_eq!(expected, actual);
        }

        // Feature: file-lock-inspector, Property 5: AppState 反映最新扫描结果
        // Validates: Requirements 3.4, 3.5
        #[test]
        fn prop_apply_scan_event_reflects_latest(
            counts in proptest::collection::vec(0usize..5usize, 1..6),
        ) {
            let fs = MockFs {
                existing: [PathBuf::from("C:/x")].into_iter().collect(),
                dirs: HashSet::new(),
            };
            let mut state = AppState::new_default();
            let id = try_add_target(&mut state, PathBuf::from("C:/x"), &fs).unwrap();

            let mut last_count = 0usize;
            for c in &counts {
                let recs: Vec<crate::detector::ProcessRecord> = (0..*c).map(|i| {
                    crate::detector::ProcessRecord {
                        pid: i as u32 + 1,
                        name: "x.exe".into(),
                        image_path: None,
                        locked_subpath: None,
                        locked_subitem_count: 1,
                        start_time: None,
                        app_type: crate::detector::AppType::Unknown,
                        is_system: false,
                        user_sid: None,
                        user_account: None,
                    }
                }).collect();
                apply_scan_event(&mut state, ScanEvent::Completed(id, recs));
                last_count = *c;
            }

            let t = state.targets.get(&id).unwrap();
            prop_assert_eq!(t.processes.len(), last_count);
            if last_count == 0 {
                prop_assert!(matches!(t.status, TargetStatus::Idle));
            } else {
                let ok = matches!(&t.status, TargetStatus::Locked { count } if *count == last_count);
                prop_assert!(ok);
            }
        }

        // Feature: file-lock-inspector, Property 10: 列表移除/清空的最终状态
        // Validates: Requirements 7.1, 7.3
        #[test]
        fn prop_remove_and_clear_idempotent(
            paths in proptest::collection::vec(arb_path(), 1..10),
        ) {
            let existing: HashSet<PathBuf> = paths.iter().cloned().collect();
            let fs = MockFs { existing, dirs: HashSet::new() };
            let mut state = AppState::new_default();
            for p in &paths {
                let _ = try_add_target(&mut state, p.clone(), &fs);
            }

            // 移除幂等性
            if let Some(&id) = state.targets.keys().next() {
                let mut s1 = state_clone(&state);
                apply(&mut s1, UiCmd::RemoveTarget(id), &fs);
                let s1_targets: Vec<_> = s1.targets.keys().copied().collect();
                let mut s2 = state_clone(&s1);
                apply(&mut s2, UiCmd::RemoveTarget(id), &fs);
                let s2_targets: Vec<_> = s2.targets.keys().copied().collect();
                prop_assert_eq!(s1_targets, s2_targets);
                prop_assert!(!s2.targets.contains_key(&id));
            }

            // 清空幂等性
            let mut s1 = state_clone(&state);
            apply(&mut s1, UiCmd::ClearAll { confirmed: true }, &fs);
            prop_assert!(s1.targets.is_empty());
            apply(&mut s1, UiCmd::ClearAll { confirmed: true }, &fs);
            prop_assert!(s1.targets.is_empty());
        }

        // Feature: file-lock-inspector, Property 12: 取消操作的状态恒等
        // Validates: Requirements 4.9
        #[test]
        fn prop_cancel_keeps_state_unchanged(
            paths in proptest::collection::vec(arb_path(), 0..6),
        ) {
            let existing: HashSet<PathBuf> = paths.iter().cloned().collect();
            let fs = MockFs { existing, dirs: HashSet::new() };
            let mut state = AppState::new_default();
            for p in &paths { let _ = try_add_target(&mut state, p.clone(), &fs); }

            let snapshot = (
                state.targets.len(),
                state.polling_interval_ms,
                state.next_id,
            );

            // ClearAll{confirmed:false} 模拟"取消"分支
            apply(&mut state, UiCmd::ClearAll { confirmed: false }, &fs);

            let after = (
                state.targets.len(),
                state.polling_interval_ms,
                state.next_id,
            );
            prop_assert_eq!(snapshot, after);
        }

        // Feature: file-lock-inspector, Property 13: 批量添加结果聚合
        // Validates: Requirements 1.5, 1.6, 1.9
        #[test]
        fn prop_batch_add_summary_aggregation(
            inputs in proptest::collection::vec(arb_path(), 1..20),
            existing_set in proptest::collection::vec(arb_path(), 0..15),
        ) {
            let existing: HashSet<PathBuf> = existing_set.into_iter().collect();
            let fs = MockFs { existing, dirs: HashSet::new() };
            let mut state = AppState::new_default();
            let total = inputs.len();
            let summary = try_add_targets(&mut state, inputs.clone(), &fs);

            // (a) 三者之和等于输入总数
            prop_assert_eq!(
                summary.added + summary.skipped_existing + summary.rejected_missing,
                total
            );

            // (b) Toast 文案严格匹配
            let msg = summary.to_toast_message();
            let expected = format!(
                "成功添加 {} 项 / 跳过 {} 项已存在 / 拒绝 {} 项不存在",
                summary.added, summary.skipped_existing, summary.rejected_missing
            );
            prop_assert_eq!(msg, expected);
        }
    }

    /// 浅拷贝 AppState（仅复制对 PBT 有意义的字段）
    fn state_clone(s: &AppState) -> AppState {
        let mut copy = AppState::new_default();
        copy.next_id = s.next_id;
        copy.polling_interval_ms = s.polling_interval_ms;
        copy.privilege = s.privilege;
        copy.windows_supported = s.windows_supported;
        for (id, t) in &s.targets {
            copy.targets.insert(
                *id,
                TargetItem {
                    id: t.id,
                    path: t.path.clone(),
                    kind: t.kind,
                    status: t.status.clone(),
                    processes: t.processes.clone(),
                    last_scanned_at: t.last_scanned_at,
                },
            );
        }
        copy
    }
}
