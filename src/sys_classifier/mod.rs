//! 系统进程分类器（is_system_process）与硬编码黑名单模块骨架。
//!
//! 完整子模块（`blacklist` 等）与 `is_system_process` 实现会在 Wave 3
//! （任务 3.x）中落地。本文件目前仅承载脚手架占位。

/// SysClassifier 层占位空 struct。
#[derive(Debug, Default, Clone, Copy)]
pub struct SysClassifierPlaceholder;

pub use SysClassifierPlaceholder as Placeholder;
