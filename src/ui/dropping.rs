//! 拖放接收（Requirement 1.3）

use eframe::egui;

use crate::state::UiCmd;

/// 每帧调用，处理拖入的文件/文件夹。
/// hovered_files 非空时显示半透明遮罩。
pub fn handle(ctx: &egui::Context, pending_cmds: &mut Vec<UiCmd>) {
    // 处理已释放的文件
    let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
    if !dropped.is_empty() {
        let paths: Vec<_> = dropped.into_iter().filter_map(|f| f.path).collect();
        if !paths.is_empty() {
            pending_cmds.push(UiCmd::AddPaths(paths));
        }
    }

    // 悬停时显示遮罩提示
    let hovered: Vec<_> = ctx.input(|i| i.raw.hovered_files.clone());
    if !hovered.is_empty() {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("drop_overlay"),
        ));
        let screen = ctx.screen_rect();
        painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));
        painter.text(
            screen.center(),
            egui::Align2::CENTER_CENTER,
            "松开以添加",
            egui::FontId::proportional(32.0),
            egui::Color32::WHITE,
        );
    }
}
