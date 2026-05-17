//! 应用状态层（AppState、TargetItem、UiCmd 等）模块。
//!
//! 子模块组织（按 Wave 2 分批落地）：
//! - `target`：`TargetId` / `TargetKind` / `TargetStatus` / `TargetItem`（task 2.1，本 task）
//! - `app_state`：`AppState` / `PrivilegeLevel` / `UiToast`（task 2.3，待实现）

pub mod target;

pub use target::*;

/// 状态层占位空 struct（兼容用，待 task 2.3 中 `AppState` 落地后移除）。
///
/// 当前仅为保持 `main.rs` 中 `let _ = state::Placeholder;` 编译通过。
#[derive(Debug, Default, Clone, Copy)]
pub struct StatePlaceholder;

pub use StatePlaceholder as Placeholder;
