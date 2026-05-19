//! 后台监控引擎（调度器、命令/事件枚举、Clock 抽象）模块。
//!
//! 子模块：
//! - `clock`：`Clock` trait 与 `SystemClock` 真实实现（任务 7.1）。
//! - `scheduler`：`SchedulerState` 状态机（任务 9.2）。

pub mod clock;
pub mod engine;
pub mod scheduler;

pub use clock::{Clock, FakeClock, SystemClock};
pub use engine::MonitorEngine;
pub use scheduler::SchedulerState;

use std::path::PathBuf;

use crate::detector::ProcessRecord;
use crate::error::ScanFailure;
use crate::state::target::{TargetId, TargetKind};

/// UI 层 → Monitor_Engine 的命令枚举。
#[derive(Debug)]
pub enum MonitorCmd {
    /// 添加新的监控目标。
    AddTarget {
        id: TargetId,
        path: PathBuf,
        kind: TargetKind,
    },
    /// 移除指定监控目标。
    RemoveTarget(TargetId),
    /// 设置轮询间隔（毫秒）。
    SetInterval(u32),
    /// 立即触发指定目标的扫描（忽略距上次扫描的间隔）。
    TriggerImmediate(TargetId),
    /// 关闭 Monitor_Engine。
    Shutdown,
}

/// Monitor_Engine → UI 层的扫描事件枚举。
#[derive(Debug)]
pub enum ScanEvent {
    /// 指定目标开始扫描。
    Started(TargetId),
    /// 指定目标扫描完成，附带锁定进程列表。
    Completed(TargetId, Vec<ProcessRecord>),
    /// 指定目标扫描失败，附带失败原因。
    Failed(TargetId, ScanFailure),
}
