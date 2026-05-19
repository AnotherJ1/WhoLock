//! 二次确认对话框（Requirements 4.2, 4.8, 7.2, 7.3）

use crate::i18n::{format_n, t, Key};
use crate::state::target::TargetId;
use crate::state::UiCmd;
use eframe::egui;

/// 强制结束进程二次确认对话框状态
#[derive(Default)]
pub struct TerminateConfirmDialog {
    pub open: bool,
    pub pid: u32,
    pub process_name: String,
    pub target_id: Option<TargetId>,
    pub start_time: Option<i64>,
}

impl TerminateConfirmDialog {
    pub fn open(&mut self, pid: u32, name: &str, target_id: TargetId, start_time: Option<i64>) {
        self.open = true;
        self.pid = pid;
        self.process_name = name.to_string();
        self.target_id = Some(target_id);
        self.start_time = start_time;
    }

    /// 渲染对话框，返回用户动作
    pub fn show(&mut self, ctx: &egui::Context, pending_cmds: &mut Vec<UiCmd>) {
        if !self.open {
            return;
        }

        // 灰幕遮罩
        egui::Area::new(egui::Id::new("modal_backdrop"))
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
            });

        egui::Window::new(t(Key::DlgTermTitle))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(t(Key::DlgTermMessage));
                ui.label(
                    egui::RichText::new(format!("{} (PID {})", self.process_name, self.pid))
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(t(Key::DlgTermWarning))
                        .color(egui::Color32::YELLOW)
                        .small(),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(t(Key::DlgTermConfirm))
                                .color(egui::Color32::from_rgb(0xD9, 0x3A, 0x3A)),
                        )
                        .clicked()
                    {
                        if let Some(target_id) = self.target_id {
                            pending_cmds.push(UiCmd::ConfirmTerminate {
                                pid: self.pid,
                                process_name: self.process_name.clone(),
                                target_id,
                                start_time: self.start_time,
                            });
                        }
                        self.open = false;
                    }
                    if ui.button(t(Key::BtnCancel)).clicked() {
                        self.open = false;
                    }
                });
            });
    }
}

/// 清空列表二次确认对话框
#[derive(Default)]
pub struct ClearAllDialog {
    pub open: bool,
    pub count: usize,
}

impl ClearAllDialog {
    pub fn show(&mut self, ctx: &egui::Context, pending_cmds: &mut Vec<UiCmd>) {
        if !self.open {
            return;
        }

        egui::Area::new(egui::Id::new("clear_modal_backdrop"))
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
            });

        egui::Window::new(t(Key::DlgClearTitle))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format_n(t(Key::DlgClearMessageFmt), self.count));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t(Key::DlgClearConfirm)).clicked() {
                        pending_cmds.push(UiCmd::ClearAll { confirmed: true });
                        self.open = false;
                    }
                    if ui.button(t(Key::BtnCancel)).clicked() {
                        self.open = false;
                    }
                });
            });
    }
}
