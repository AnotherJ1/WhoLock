//! UI 层（egui 渲染、对话框、拖放）模块骨架。
//!
//! 完整子模块（`target_list` / `process_row` / `status_bar` / `dialogs` /
//! `dropping`）会在 Wave 6（任务 12.x ~ 15.x）中按设计文档 4.2 节落地。
//!
//! 本文件目前仅承载脚手架占位，确保 `cargo check` 在 Wave 1 即可通过。

/// UI 层占位空 struct。
///
/// 后续任务会替换为 `FileLockInspectorApp` 的相关 UI 组件，
/// 此处保留一个零成本类型用于在 `main.rs` 中引用模块、避免 `dead_code` 警告。
#[derive(Debug, Default, Clone, Copy)]
pub struct UiPlaceholder;

pub use UiPlaceholder as Placeholder;
