//! 底部状态栏：权限状态、提权按钮、轮询间隔（Requirements 6.1, 6.2, 6.5, 3.3）

use crate::i18n::{t, Key};
use crate::state::app_state::{PrivilegeLevel, UiToast};
use crate::state::{AppState, UiCmd};
use eframe::egui;

const COLOR_GREEN: egui::Color32 = egui::Color32::from_rgb(0x22, 0xC5, 0x5E);
const COLOR_AMBER: egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B);
const COLOR_RED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
const COLOR_BLUE: egui::Color32 = egui::Color32::from_rgb(0x3B, 0x82, 0xF6);
const COLOR_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x9C, 0xA3, 0xAF);

/// 渲染底部状态栏
pub fn show(ui: &mut egui::Ui, state: &AppState, pending_cmds: &mut Vec<UiCmd>) {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // 权限状态徽章
                match state.privilege {
                    PrivilegeLevel::Standard => {
                        render_pill(ui, t(Key::PrivStandard), COLOR_AMBER);
                        ui.add_space(8.0);
                        if ui
                            .small_button(
                                egui::RichText::new(t(Key::BtnRestartAdmin))
                                    .size(13.0)
                                    .color(COLOR_BLUE)
                                    .strong(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            pending_cmds
                                .push(UiCmd::ShowToast(UiToast::info(t(Key::ToastElevating))));
                            match crate::elevation::restart_as_admin() {
                                Ok(()) => {}
                                Err(crate::error::AppError::SelfExit) => {
                                    std::process::exit(0);
                                }
                                Err(e) => {
                                    pending_cmds
                                        .push(UiCmd::ShowToast(UiToast::error(e.to_string())));
                                }
                            }
                        }
                    }
                    PrivilegeLevel::Elevated => {
                        render_pill(ui, t(Key::PrivAdmin), COLOR_GREEN);
                    }
                }

                ui.add_space(16.0);
                ui.label(egui::RichText::new("│").color(egui::Color32::from_rgb(0x37, 0x40, 0x4D)));
                ui.add_space(8.0);

                // 轮询间隔
                ui.label(
                    egui::RichText::new(t(Key::LabelRefresh))
                        .size(13.0)
                        .color(COLOR_TEXT_DIM),
                );
                let intervals = [1000u32, 2000, 5000, 10000];
                let labels = ["1s", "2s", "5s", "10s"];
                for (ms, label) in intervals.iter().zip(labels.iter()) {
                    let selected = state.polling_interval_ms == *ms;
                    let text = if selected {
                        egui::RichText::new(*label)
                            .color(COLOR_BLUE)
                            .strong()
                            .size(13.0)
                    } else {
                        egui::RichText::new(*label).color(COLOR_TEXT_DIM).size(13.0)
                    };
                    if ui
                        .selectable_label(selected, text)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && !selected
                    {
                        pending_cmds.push(UiCmd::SetInterval(*ms));
                    }
                }

                // Toast 提示靠右显示
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(toast) = &state.last_error {
                        if ui
                            .small_button("✕")
                            .on_hover_text(t(Key::ToastClosing))
                            .clicked()
                        {
                            pending_cmds.push(UiCmd::DismissToast);
                        }
                        ui.add_space(4.0);
                        let color = if toast.is_error {
                            COLOR_RED
                        } else {
                            COLOR_GREEN
                        };
                        render_pill(ui, &toast.message, color);
                    }
                });
            });
        });
}

fn render_pill(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.5)))
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(10.0, 3.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(13.0).color(color).strong());
        });
}
