// File_Lock_Inspector 入口（脚手架阶段）
//
// 本文件在任务 1.2 中扩展：仅声明 design 4.2 节定义的六个一级模块，
// 让 `cargo check` 能完整解析整棵模块树。
// 真正的窗口、日志初始化、配置加载等会在后续任务（1.4 / 1.5 等）中接入。
//
// 仅在 windows 平台下编译（design 决策：Win32 限定）；其他平台下保留空 main，
// 便于 IDE/工具链在跨平台环境中也能跑 `cargo check`。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// --- 模块声明（design 4.2「文件结构」） ---
mod detector;
mod monitor;
mod process_info;
mod state;
mod sys_classifier;
mod ui;
mod error;

fn main() {
    // 触发各模块的占位 struct 实例化，避免 `dead_code` 警告，
    // 并让链接器确认所有 mod.rs 都参与编译。
    let _ = ui::Placeholder;
    let _ = state::Placeholder;
    // detector 模块的占位已移除，ProcessRecord/AppType 已就位（任务 2.2）。
    let _ = process_info::Placeholder;
    let _ = sys_classifier::Placeholder;
    // monitor 模块的占位已移除，Clock trait + SystemClock 已就位（任务 7.1）。
    let _ = monitor::SystemClock;
}
