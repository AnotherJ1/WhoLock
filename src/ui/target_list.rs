//! 目标项列表视图（Requirements 7.1, 7.5, 1.5, 1.6）

use eframe::egui;

use crate::i18n::{t, Key};
use crate::state::target::TargetStatus;
use crate::state::{AppState, UiCmd};

// 现代化设计 token
const COLOR_GREEN: egui::Color32 = egui::Color32::from_rgb(0x22, 0xC5, 0x5E); // success
const COLOR_AMBER: egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B); // warning / locked
const COLOR_RED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44); // danger
const COLOR_BLUE: egui::Color32 = egui::Color32::from_rgb(0x3B, 0x82, 0xF6); // info / scanning
const COLOR_SLATE: egui::Color32 = egui::Color32::from_rgb(0x64, 0x74, 0x8B); // neutral / pending
const COLOR_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x9C, 0xA3, 0xAF);
const COLOR_CARD_BG: egui::Color32 = egui::Color32::from_rgb(0x1A, 0x1F, 0x26);
const COLOR_CARD_STROKE: egui::Color32 = egui::Color32::from_rgb(0x2A, 0x31, 0x3C);

/// 渲染目标列表面板（CentralPanel + ScrollArea）
pub fn show(ui: &mut egui::Ui, state: &AppState, pending_cmds: &mut Vec<UiCmd>) {
    if state.targets.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("📂").size(48.0).color(COLOR_SLATE));
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(t(Key::EmptyDropHint))
                    .size(18.0)
                    .color(COLOR_TEXT_DIM)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(t(Key::EmptySubHint))
                    .size(14.0)
                    .color(COLOR_SLATE),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(8.0);
            let ids: Vec<_> = state.targets.keys().copied().collect();
            for id in ids {
                let Some(target) = state.targets.get(&id) else {
                    continue;
                };

                // 卡片容器
                egui::Frame::none()
                    .fill(COLOR_CARD_BG)
                    .stroke(egui::Stroke::new(1.0, COLOR_CARD_STROKE))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 状态徽章（彩色圆点 + 文字）
                            render_status_badge(ui, &target.status);

                            ui.add_space(6.0);

                            // 路径（等宽字体，强调显示）
                            ui.label(
                                egui::RichText::new(target.path.display().to_string())
                                    .monospace()
                                    .size(15.0),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // 移除按钮（圆形）
                                    let btn =
                                        egui::Button::new(egui::RichText::new("✕").size(14.0))
                                            .min_size(egui::vec2(28.0, 28.0))
                                            .rounding(egui::Rounding::same(6.0));
                                    if ui.add(btn).on_hover_text(t(Key::TooltipRemove)).clicked() {
                                        pending_cmds.push(UiCmd::RemoveTarget(id));
                                    }
                                },
                            );
                        });

                        // 展开显示进程记录
                        if !target.processes.is_empty() {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);
                            for record in &target.processes {
                                crate::ui::process_row::show(ui, record, id, pending_cmds);
                            }
                        }
                    });

                ui.add_space(8.0);
            }
        });
}

/// 渲染状态徽章：彩色圆点 + 状态文字
fn render_status_badge(ui: &mut egui::Ui, status: &TargetStatus) {
    let (text, color): (&str, egui::Color32) = match status {
        TargetStatus::Pending => (t(Key::StatusPending), COLOR_SLATE),
        TargetStatus::Scanning => (t(Key::StatusScanning), COLOR_BLUE),
        TargetStatus::Idle => (t(Key::StatusIdle), COLOR_GREEN),
        TargetStatus::Locked { count } => {
            let s = format!(
                "{} · {} {}",
                t(Key::StatusLockedPrefix),
                count,
                t(Key::ProcessUnit)
            );
            render_dot_label(ui, &s, COLOR_AMBER);
            return;
        }
        TargetStatus::Failed { .. } => (t(Key::StatusFailed), COLOR_RED),
        TargetStatus::AccessDenied => (t(Key::StatusAccessDenied), COLOR_AMBER),
    };
    render_dot_label(ui, text, color);
}

fn render_dot_label(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    // 用 RichText.background_color 模拟 pill：圆点 + 文字 + 半透明背景
    egui::Frame::none()
        .fill(color.linear_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.5)))
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // 圆点 — 使用大号实心圆字符 ●
                ui.label(egui::RichText::new("●").color(color).size(10.0));
                ui.label(egui::RichText::new(text).color(color).size(13.0).strong());
            });
        });
}
