//! Lock_Detector：Restart Manager 调用封装与 ProcessRecord 装配模块骨架。
//!
//! 完整子模块（`restart_manager` 等）会在 Wave 4A（任务 4.x）和 Wave 5A
//! （任务 8.x）中实现。本文件目前承载脚手架占位与已实现的子模块导出。

pub mod enumerate;

pub use enumerate::enumerate_direct_children;

/// Detector 层占位空 struct。
#[derive(Debug, Default, Clone, Copy)]
pub struct DetectorPlaceholder;

pub use DetectorPlaceholder as Placeholder;
