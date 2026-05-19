//! 单条 ProcessRecord 行渲染（Requirements 4.1, 5.3, 5.4）

use eframe::egui;

use crate::detector::ProcessRecord;
use crate::i18n::{t, Key};
use crate::state::app_state::UiToast;
use crate::state::target::TargetId;
use crate::state::UiCmd;

const COLOR_RED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
const COLOR_RED_HOVER: egui::Color32 = egui::Color32::from_rgb(0xDC, 0x26, 0x26);
const COLOR_SLATE: egui::Color32 = egui::Color32::from_rgb(0x64, 0x74, 0x8B);
const COLOR_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x9C, 0xA3, 0xAF);
const COLOR_TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0xE8, 0xEA, 0xED);
const COLOR_PILL_SYS: egui::Color32 = egui::Color32::from_rgb(0x37, 0x40, 0x4D);

/// 系统进程处置建议（取自 i18n）
fn system_guide() -> &'static str {
    t(Key::SystemGuideText)
}

/// 渲染单条 `ProcessRecord` 行。
pub fn show(
    ui: &mut egui::Ui,
    record: &ProcessRecord,
    target_id: TargetId,
    pending_cmds: &mut Vec<UiCmd>,
) {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // PID 徽章
                egui::Frame::none()
                    .fill(COLOR_PILL_SYS)
                    .rounding(egui::Rounding::same(4.0))
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("PID {}", record.pid))
                                .size(13.0)
                                .monospace()
                                .color(COLOR_TEXT_DIM),
                        );
                    });

                ui.add_space(2.0);

                // 进程名（强调）
                ui.label(
                    egui::RichText::new(&record.name)
                        .size(15.0)
                        .strong()
                        .color(COLOR_TEXT_PRIMARY),
                );

                // 用户账户徽章
                if let Some(account) = &record.user_account {
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(1.0, COLOR_SLATE))
                        .rounding(egui::Rounding::same(4.0))
                        .inner_margin(egui::Margin::symmetric(6.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(account)
                                    .size(12.0)
                                    .color(COLOR_TEXT_DIM),
                            );
                        });
                }

                // 文件夹模式：占用子项数量
                if record.locked_subitem_count > 1 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} {}",
                            t(Key::SubitemPrefix),
                            record.locked_subitem_count,
                            t(Key::SubitemSuffix)
                        ))
                        .size(13.0)
                        .color(COLOR_TEXT_DIM),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if record.is_system {
                        // 系统进程：禁止结束
                        if ui
                            .small_button(egui::RichText::new(t(Key::BtnViewGuide)).size(13.0))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            let summary = system_guide()
                                .lines()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join(" | ");
                            pending_cmds.push(UiCmd::ShowToast(UiToast::info(summary)));
                        }
                        ui.add_space(4.0);
                        // 系统进程标识 pill
                        egui::Frame::none()
                            .fill(COLOR_PILL_SYS)
                            .stroke(egui::Stroke::new(1.0, COLOR_SLATE))
                            .rounding(egui::Rounding::same(999.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(t(Key::LabelSystemProcess))
                                        .size(12.0)
                                        .color(COLOR_TEXT_DIM),
                                );
                            });
                    } else {
                        // 非系统进程：红色"强制结束"按钮
                        let resp = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(t(Key::BtnTerminate))
                                        .color(egui::Color32::WHITE)
                                        .size(13.0)
                                        .strong(),
                                )
                                .fill(if ui.rect_contains_pointer(ui.max_rect()) {
                                    COLOR_RED_HOVER
                                } else {
                                    COLOR_RED
                                })
                                .rounding(egui::Rounding::same(6.0))
                                .min_size(egui::vec2(80.0, 28.0)),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(format!(
                                "{}: {} (PID {})",
                                t(Key::TooltipTerminate),
                                record.name,
                                record.pid
                            ));

                        if resp.clicked() {
                            pending_cmds.push(UiCmd::OpenTerminateDialog {
                                pid: record.pid,
                                process_name: record.name.clone(),
                                target_id,
                                start_time: record.start_time,
                            });
                        }
                    }
                });
            });

            // 第二行：image_path（如有）
            if let Some(path) = &record.image_path {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("📄").size(11.0).color(COLOR_SLATE));
                    ui.label(
                        egui::RichText::new(path.display().to_string())
                            .size(12.0)
                            .color(COLOR_SLATE),
                    );
                });
            }
        });
}
