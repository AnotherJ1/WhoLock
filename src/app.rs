//! FileLockInspectorApp：eframe::App 实现，连接 UI 与后台 Monitor Engine
//!
//! Requirements: UI 层基础骨架

use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use eframe::egui;

use crate::i18n::{self, format_pid, t, Key, Language};
use crate::monitor::{MonitorCmd, ScanEvent};
use crate::state::target::{TargetId, TargetKind};
use crate::state::{apply, apply_scan_event, AppState, RealFsProbe, UiCmd};

pub struct FileLockInspectorApp {
    pub state: Arc<Mutex<AppState>>,
    pub cmd_tx: Sender<MonitorCmd>,
    pub event_rx: Receiver<ScanEvent>,
    pub clear_dialog: crate::ui::dialogs::ClearAllDialog,
    pub terminate_dialog: crate::ui::dialogs::TerminateConfirmDialog,
}

impl FileLockInspectorApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        state: Arc<Mutex<AppState>>,
        cmd_tx: Sender<MonitorCmd>,
        event_rx: Receiver<ScanEvent>,
    ) -> Self {
        // 自定义现代深色主题
        apply_modern_theme(&cc.egui_ctx);
        // 加载系统中文字体，避免界面中文乱码
        install_chinese_fonts(&cc.egui_ctx);
        Self {
            state,
            cmd_tx,
            event_rx,
            clear_dialog: Default::default(),
            terminate_dialog: Default::default(),
        }
    }

    /// 把 UiCmd 应用到 AppState，并向 Monitor 派发必要命令
    fn dispatch_cmd(&mut self, cmd: UiCmd) {
        // 先处理需要 self 引用且不需要 state 锁的命令
        match &cmd {
            UiCmd::OpenTerminateDialog {
                pid,
                process_name,
                target_id,
                start_time,
            } => {
                self.terminate_dialog
                    .open(*pid, process_name, *target_id, *start_time);
                return;
            }
            UiCmd::ConfirmTerminate {
                pid,
                process_name,
                target_id,
                start_time,
            } => {
                use crate::state::app_state::UiToast;
                let pid = *pid;
                let target_id = *target_id;
                let process_name = process_name.clone();
                let start_time_ft = start_time.map(|t| windows::Win32::Foundation::FILETIME {
                    dwLowDateTime: (t & 0xFFFFFFFF) as u32,
                    dwHighDateTime: ((t >> 32) & 0xFFFFFFFF) as u32,
                });
                let cmd_tx = self.cmd_tx.clone();
                let state_arc = self.state.clone();

                // 立即显示"正在结束"提示
                if let Ok(mut s) = self.state.lock() {
                    s.last_error = Some(UiToast::info(format_pid(
                        t(Key::ToastTermInProgressFmt),
                        &process_name,
                        pid,
                    )));
                }

                std::thread::spawn(move || {
                    let result =
                        crate::terminator::force_terminate_with_timeout(pid, start_time_ft);
                    use crate::error::TerminateError;
                    let toast = match result {
                        Ok(()) => {
                            let _ = cmd_tx.send(MonitorCmd::TriggerImmediate(target_id));
                            UiToast::info(format_pid(
                                t(Key::ToastTermSuccessFmt),
                                &process_name,
                                pid,
                            ))
                        }
                        Err(TerminateError::AlreadyExited) => {
                            let _ = cmd_tx.send(MonitorCmd::TriggerImmediate(target_id));
                            UiToast::info(format_pid(
                                t(Key::ToastTermAlreadyExitedFmt),
                                &process_name,
                                pid,
                            ))
                        }
                        Err(TerminateError::AccessDenied) => {
                            UiToast::error(t(Key::ToastTermAccessDenied).to_string())
                        }
                        Err(TerminateError::Timeout) => {
                            UiToast::error(t(Key::ToastTermTimeout).to_string())
                        }
                        Err(TerminateError::SystemProtected) => {
                            UiToast::error(t(Key::ToastTermSystemProtected).to_string())
                        }
                        Err(TerminateError::StalePid) => UiToast::error(format_pid(
                            t(Key::ToastTermStalePidFmt),
                            &process_name,
                            pid,
                        )),
                        Err(other) => UiToast::error(other.to_string()),
                    };
                    if let Ok(mut s) = state_arc.lock() {
                        s.last_error = Some(toast);
                    }
                });
                return;
            }
            _ => {}
        }

        // 其余命令：先 lock state，根据需要派发到 monitor
        let mut state = self.state.lock().unwrap();
        match &cmd {
            UiCmd::AddPaths(paths) => {
                let fs = RealFsProbe;
                for path in paths {
                    if let Ok(id) = crate::state::try_add_target(&mut state, path.clone(), &fs) {
                        let kind = if path.is_dir() {
                            TargetKind::Directory
                        } else {
                            TargetKind::File
                        };
                        let _ = self.cmd_tx.send(MonitorCmd::AddTarget {
                            id,
                            path: path.clone(),
                            kind,
                        });
                    }
                }
                return;
            }
            UiCmd::RemoveTarget(id) => {
                let _ = self.cmd_tx.send(MonitorCmd::RemoveTarget(*id));
            }
            UiCmd::ClearAll { confirmed: true } => {
                let ids: Vec<TargetId> = state.targets.keys().copied().collect();
                for id in ids {
                    let _ = self.cmd_tx.send(MonitorCmd::RemoveTarget(id));
                }
            }
            UiCmd::SetInterval(ms) => {
                let _ = self.cmd_tx.send(MonitorCmd::SetInterval(*ms));
            }
            _ => {}
        }
        apply(&mut state, cmd, &RealFsProbe);
    }
}

impl eframe::App for FileLockInspectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. 排空后台扫描事件
        {
            let mut state = self.state.lock().unwrap();
            while let Ok(ev) = self.event_rx.try_recv() {
                apply_scan_event(&mut state, ev);
            }
        }

        // 2. 请求定时重绘（100ms ≈ 10fps，足够 UI 响应）
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        let mut pending_cmds: Vec<UiCmd> = Vec::new();

        // 3. 拖放处理
        crate::ui::dropping::handle(ctx, &mut pending_cmds);

        // 4. 顶部工具栏
        egui::TopBottomPanel::top("toolbar")
            .show_separator_line(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 应用标题
                            ui.label(
                                egui::RichText::new(t(Key::AppTitle))
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(0xE8, 0xEA, 0xED)),
                            );
                            ui.add_space(20.0);

                            // 主操作按钮（蓝色强调）
                            let primary_btn = |label: &str| -> egui::Button<'_> {
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .size(14.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(0x3B, 0x82, 0xF6))
                                .rounding(egui::Rounding::same(6.0))
                                .min_size(egui::vec2(96.0, 32.0))
                            };

                            if ui
                                .add(primary_btn(t(Key::BtnAddFile)))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                if let Some(paths) = rfd::FileDialog::new().pick_files() {
                                    if !paths.is_empty() {
                                        pending_cmds.push(UiCmd::AddPaths(paths));
                                    }
                                }
                            }
                            if ui
                                .add(primary_btn(t(Key::BtnAddFolder)))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                if let Some(paths) = rfd::FileDialog::new().pick_folders() {
                                    if !paths.is_empty() {
                                        pending_cmds.push(UiCmd::AddPaths(paths));
                                    }
                                }
                            }

                            ui.add_space(8.0);

                            // 次要按钮（无填充）
                            {
                                let count = self.state.lock().unwrap().targets.len();
                                let secondary_btn = |label: &str| -> egui::Button<'_> {
                                    egui::Button::new(
                                        egui::RichText::new(label)
                                            .size(14.0)
                                            .color(egui::Color32::from_rgb(0x9C, 0xA3, 0xAF)),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(0x37, 0x40, 0x4D),
                                    ))
                                    .rounding(egui::Rounding::same(6.0))
                                    .min_size(egui::vec2(80.0, 32.0))
                                };
                                if ui
                                    .add(secondary_btn(t(Key::BtnClearList)))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                    && count > 0
                                {
                                    self.clear_dialog.open = true;
                                    self.clear_dialog.count = count;
                                }
                                if ui
                                    .add(secondary_btn(t(Key::BtnOpenLogDir)))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    if let Some(log_dir) = crate::logging::log_dir() {
                                        let _ = std::process::Command::new("explorer")
                                            .arg(log_dir)
                                            .spawn();
                                    }
                                }
                                // 语言切换按钮（显示"切换到的目标语言"名称）
                                if ui
                                    .add(secondary_btn(t(Key::BtnLanguage)))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    let next = match i18n::current_language() {
                                        Language::En => Language::Zh,
                                        Language::Zh => Language::En,
                                    };
                                    i18n::set_language(next);
                                    // 持久化语言选择
                                    let mut cfg = crate::config::AppConfig::load();
                                    cfg.language = next;
                                    cfg.save();
                                }
                            }

                            // 右侧：目标计数
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let count = self.state.lock().unwrap().targets.len();
                                    if count > 0 {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} {}",
                                                count,
                                                t(Key::TargetCountSuffix)
                                            ))
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(0x9C, 0xA3, 0xAF)),
                                        );
                                    }
                                },
                            );
                        });
                    });
            });

        // 5. 底部状态栏
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let state = self.state.lock().unwrap();
            crate::ui::status_bar::show(ui, &state, &mut pending_cmds);
        });

        // 6. 中央目标列表
        egui::CentralPanel::default().show(ctx, |ui| {
            let state = self.state.lock().unwrap();
            crate::ui::target_list::show(ui, &state, &mut pending_cmds);
        });

        // 7. 对话框
        self.clear_dialog.show(ctx, &mut pending_cmds);
        self.terminate_dialog.show(ctx, &mut pending_cmds);

        // 8. 派发 pending 命令
        for cmd in pending_cmds {
            self.dispatch_cmd(cmd);
        }
    }
}

/// 应用现代深色主题（配色 / 圆角 / 间距 / 字号）
fn apply_modern_theme(ctx: &egui::Context) {
    use egui::{
        epaint::Shadow,
        style::{Selection, WidgetVisuals, Widgets},
        Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals,
    };

    // ---------------------------------------------------------------------
    // 配色系统（现代深色）
    // ---------------------------------------------------------------------
    let bg_base = Color32::from_rgb(0x0E, 0x11, 0x16); // #0E1116
    let bg_panel = Color32::from_rgb(0x14, 0x18, 0x1F); // #14181F
    let bg_card = Color32::from_rgb(0x1A, 0x1F, 0x26); // #1A1F26
    let bg_card_hover = Color32::from_rgb(0x22, 0x28, 0x31);
    let bg_card_active = Color32::from_rgb(0x2A, 0x31, 0x3C);
    let stroke_subtle = Color32::from_rgb(0x2A, 0x31, 0x3C);
    let stroke_default = Color32::from_rgb(0x37, 0x40, 0x4D);
    let text_primary = Color32::from_rgb(0xE8, 0xEA, 0xED); // #E8EAED
    let text_secondary = Color32::from_rgb(0x9C, 0xA3, 0xAF); // #9CA3AF
    let accent = Color32::from_rgb(0x3B, 0x82, 0xF6); // #3B82F6 (blue-500)
    let accent_hover = Color32::from_rgb(0x60, 0xA5, 0xFA); // blue-400

    let mut visuals = Visuals::dark();

    // 整体背景
    visuals.window_fill = bg_panel;
    visuals.panel_fill = bg_base;
    visuals.extreme_bg_color = bg_base;
    visuals.faint_bg_color = bg_panel;
    visuals.code_bg_color = bg_card;

    // 边框
    visuals.window_stroke = Stroke::new(1.0, stroke_subtle);
    visuals.window_rounding = Rounding::same(8.0);
    visuals.window_shadow = Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(80),
    };
    visuals.popup_shadow = Shadow {
        offset: egui::vec2(0.0, 2.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(60),
    };

    // 选择 / 强调
    visuals.selection = Selection {
        bg_fill: accent.linear_multiply(0.3),
        stroke: Stroke::new(1.0, accent),
    };
    visuals.hyperlink_color = accent_hover;

    // Widget 状态
    visuals.widgets = Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: bg_panel,
            weak_bg_fill: bg_panel,
            bg_stroke: Stroke::new(1.0, stroke_subtle),
            fg_stroke: Stroke::new(1.0, text_secondary),
            rounding: Rounding::same(6.0),
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            bg_fill: bg_card,
            weak_bg_fill: bg_card,
            bg_stroke: Stroke::new(1.0, stroke_subtle),
            fg_stroke: Stroke::new(1.5, text_primary),
            rounding: Rounding::same(6.0),
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            bg_fill: bg_card_hover,
            weak_bg_fill: bg_card_hover,
            bg_stroke: Stroke::new(1.0, stroke_default),
            fg_stroke: Stroke::new(1.5, text_primary),
            rounding: Rounding::same(6.0),
            expansion: 1.0,
        },
        active: WidgetVisuals {
            bg_fill: bg_card_active,
            weak_bg_fill: bg_card_active,
            bg_stroke: Stroke::new(1.0, accent),
            fg_stroke: Stroke::new(1.5, text_primary),
            rounding: Rounding::same(6.0),
            expansion: 1.0,
        },
        open: WidgetVisuals {
            bg_fill: bg_card_active,
            weak_bg_fill: bg_card_active,
            bg_stroke: Stroke::new(1.0, accent),
            fg_stroke: Stroke::new(1.5, text_primary),
            rounding: Rounding::same(6.0),
            expansion: 0.0,
        },
    };

    // 默认文字色
    visuals.override_text_color = Some(text_primary);

    ctx.set_visuals(visuals);

    // ---------------------------------------------------------------------
    // 间距系统（基于默认 Spacing 修改，避免字段不全）
    // ---------------------------------------------------------------------
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.window_margin = egui::Margin::symmetric(16.0, 12.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.menu_margin = egui::Margin::same(8.0);
        style.spacing.indent = 18.0;
        style.spacing.interact_size = egui::vec2(40.0, 32.0);
        style.spacing.icon_width = 16.0;
        style.spacing.icon_spacing = 6.0;
        // 字号体系（基础 16，标题 22，小 13）
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(22.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(15.0, FontFamily::Monospace),
            ),
            (
                TextStyle::Button,
                FontId::new(15.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(13.0, FontFamily::Proportional),
            ),
        ]
        .into();
    });
}

/// 加载 Windows 系统中文字体，避免界面中文乱码。
///
/// 优先级：微软雅黑 (msyh.ttc) → 宋体 (simsun.ttc) → 黑体 (simhei.ttf)
/// 加载失败时静默退化为 egui 默认字体（中文仍会显示为豆腐块）。
fn install_chinese_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};

    let candidates: &[(&str, &str)] = &[
        ("msyh", r"C:\Windows\Fonts\msyh.ttc"),
        ("msyh-bold", r"C:\Windows\Fonts\msyhbd.ttc"),
        ("simsun", r"C:\Windows\Fonts\simsun.ttc"),
        ("simhei", r"C:\Windows\Fonts\simhei.ttf"),
    ];

    let mut fonts = FontDefinitions::default();
    let mut loaded_keys: Vec<String> = Vec::new();

    for (key, path) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert((*key).to_string(), FontData::from_owned(bytes));
            loaded_keys.push((*key).to_string());
        }
    }

    if loaded_keys.is_empty() {
        tracing::warn!("未能加载任何中文字体，界面可能出现乱码");
        return;
    }

    // 把所有已加载的中文字体追加到 Proportional 与 Monospace 家族末尾，
    // 确保 ASCII 仍优先用 egui 默认字体（保持原有 UI 美观），
    // 而中文从 fallback 中查找。
    if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
        for k in &loaded_keys {
            prop.push(k.clone());
        }
    }
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        for k in &loaded_keys {
            mono.push(k.clone());
        }
    }

    ctx.set_fonts(fonts);
    tracing::info!("已加载中文字体: {:?}", loaded_keys);
}
