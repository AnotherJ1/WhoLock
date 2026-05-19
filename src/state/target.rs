//! Target_Item 相关数据模型：`TargetId` / `TargetKind` / `TargetStatus` / `TargetItem`。
//!
//! 严格遵循 design.md "Data Models" 节字段定义。
//!
//! ## 派生 trait 说明
//! - `TargetId`：标识符，需可作为 `BTreeMap` / `HashSet` 键并能持久化（serde）
//! - `TargetKind` / `TargetStatus`：可序列化以备未来 Target_List 持久化或快照导出
//! - `TargetItem`：仅派生 `Clone/Debug`——`Instant` 不实现 `serde::Serialize`，
//!   且 `last_scanned_at` 是运行期内部状态，不需要序列化（design 决定不持久化 Target_List）

use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::detector::ProcessRecord;

/// Target_Item 在 `AppState` 中的稳定标识。
///
/// 内部为单调递增 `u64`，由 `AppState::next_id()` 分配（task 2.3）。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct TargetId(pub u64);

/// Target_Item 类型：单文件或文件夹。
///
/// 用于 Lock_Detector 决策注册资源时是否需要展开直接子项（需求 1.7）。
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TargetKind {
    /// 单个文件
    File,
    /// 单个文件夹（仅枚举一级子项，不递归）
    Directory,
}

/// Target_Item 当前的检测状态。
///
/// 状态机（design Property 5）：
/// - `Pending`：刚加入 Target_List，尚未首次扫描
/// - `Scanning`：当前正在被 Monitor_Engine 检测
/// - `Idle`：检测完成，未被任何进程占用
/// - `Locked { count }`：检测完成，被 `count` 个进程占用
/// - `Failed { reason }`：检测过程中发生未预期异常
/// - `AccessDenied`：Restart Manager 返回访问拒绝，需要管理员权限
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetStatus {
    Pending,
    Scanning,
    Idle,
    Locked { count: usize },
    Failed { reason: String },
    AccessDenied,
}

/// Target_List 中的单条目标项。
///
/// 字段对应 design "Data Models" 节。`last_scanned_at` 为 `Option<Instant>`：
/// `None` 表示尚未扫描过；首次扫描完成后写入。
#[derive(Clone, Debug)]
pub struct TargetItem {
    pub id: TargetId,
    pub path: PathBuf,
    pub kind: TargetKind,
    pub status: TargetStatus,
    pub processes: Vec<ProcessRecord>,
    pub last_scanned_at: Option<Instant>,
}
