//! ProcessInfoProvider：进程详细信息（image_path / token SID / 用户账户）查询模块骨架。
//!
//! 完整子模块（`token` 等）与 `snapshot(pid)` 实现会在 Wave 4B（任务 5.x）中落地。
//! 本文件目前仅承载脚手架占位。

/// ProcessInfo 层占位空 struct。
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessInfoPlaceholder;

pub use ProcessInfoPlaceholder as Placeholder;
