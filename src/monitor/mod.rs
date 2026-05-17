//! 后台监控引擎（调度器、命令/事件枚举、Clock 抽象）模块。
//!
//! 当前已落地的子模块：
//! - `clock`：`Clock` trait 与 `SystemClock` 真实实现（任务 7.1）。
//!
//! 后续会陆续加入 `scheduler`（任务 9.2）、`MonitorCmd`/`ScanEvent`（任务 9.1）等。

pub mod clock;

pub use clock::{Clock, SystemClock};
