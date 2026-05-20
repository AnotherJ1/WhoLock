//! 国际化（i18n）模块。
//!
//! 提供英语（默认）和中文两种语言的 UI 文案。
//! 通过 `t!()` 宏访问当前语言的翻译。
//!
//! 默认语言为英语；用户可通过 UI 切换到中文（持久化到 AppConfig）。

use std::sync::atomic::{AtomicU8, Ordering};

/// 支持的语言枚举（按 enum discriminant 顺序：0=En, 1=Zh）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Language {
    /// English
    En = 0,
    /// 简体中文（默认）
    #[default]
    Zh = 1,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Zh => "简体中文",
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Language::Zh,
            _ => Language::En,
        }
    }
}

/// 全局当前语言（用 AtomicU8 实现无锁读）。默认 1 = 简体中文。
static CURRENT_LANG: AtomicU8 = AtomicU8::new(1);

/// 设置当前语言（线程安全）。
pub fn set_language(lang: Language) {
    CURRENT_LANG.store(lang as u8, Ordering::Relaxed);
}

/// 获取当前语言。
pub fn current_language() -> Language {
    Language::from_u8(CURRENT_LANG.load(Ordering::Relaxed))
}

/// 翻译键 → (英文, 中文) 映射。
///
/// 添加新文案时同时填写两种语言。`t(Key)` 返回 `&'static str`。
#[derive(Debug, Clone, Copy)]
pub enum Key {
    // App
    AppTitle,
    TargetCountSuffix,

    // Toolbar
    BtnAddFile,
    BtnAddFolder,
    BtnClearList,
    BtnOpenLogDir,
    BtnLanguage,

    // Empty state
    EmptyDropHint,
    EmptySubHint,

    // Status badges
    StatusPending,
    StatusScanning,
    StatusIdle,
    StatusLockedPrefix, // followed by " · N <ProcessUnit>"
    StatusFailed,
    StatusAccessDenied,
    ProcessUnit, // "processes" / "个进程"

    // Process row
    BtnViewGuide,
    BtnTerminate,
    LabelSystemProcess,
    SubitemPrefix,    // "occupies" / "占用"
    SubitemSuffix,    // "child item(s)" / "个子项"
    TooltipTerminate, // "Terminate {} (PID {})" / "结束 {} (PID {})"
    TooltipRemove,

    // Status bar
    PrivStandard,
    PrivAdmin,
    BtnRestartAdmin,
    LabelRefresh,
    ToastClosing,

    // Confirm dialogs
    DlgTermTitle,
    DlgTermMessage,
    DlgTermWarning,
    DlgTermConfirm,
    DlgClearTitle,
    DlgClearMessageFmt, // "Will remove {N} target(s). Continue?" / "将清空全部 {N} 项..."
    DlgClearConfirm,
    BtnCancel,

    // Toasts (terminate result)
    ToastElevating,
    ToastTermInProgressFmt,
    ToastTermSuccessFmt,
    ToastTermAlreadyExitedFmt,
    ToastTermAccessDenied,
    ToastTermTimeout,
    ToastTermSystemProtected,
    ToastTermStalePidFmt,

    // Errors / messages
    MsgUnsupportedOs,

    // System process guide (multiline)
    SystemGuideText,
}

/// 返回当前语言下 `key` 对应的翻译字符串。
pub fn t(key: Key) -> &'static str {
    match (key, current_language()) {
        // ---------------- App ----------------
        (Key::AppTitle, Language::En) => "🔍 File Lock Inspector",
        (Key::AppTitle, Language::Zh) => "🔍 文件占用查询",
        (Key::TargetCountSuffix, Language::En) => "target(s) monitored",
        (Key::TargetCountSuffix, Language::Zh) => "个目标监控中",

        // ---------------- Toolbar ----------------
        (Key::BtnAddFile, Language::En) => "+ Add File",
        (Key::BtnAddFile, Language::Zh) => "+ 添加文件",
        (Key::BtnAddFolder, Language::En) => "+ Add Folder",
        (Key::BtnAddFolder, Language::Zh) => "+ 添加文件夹",
        (Key::BtnClearList, Language::En) => "Clear List",
        (Key::BtnClearList, Language::Zh) => "清空列表",
        (Key::BtnOpenLogDir, Language::En) => "Open Log Folder",
        (Key::BtnOpenLogDir, Language::Zh) => "打开日志目录",
        (Key::BtnLanguage, Language::En) => "中文",
        (Key::BtnLanguage, Language::Zh) => "English",

        // ---------------- Empty state ----------------
        (Key::EmptyDropHint, Language::En) => "Drop files or folders here",
        (Key::EmptyDropHint, Language::Zh) => "拖放文件或文件夹到此处",
        (Key::EmptySubHint, Language::En) => "Or use the buttons above to add targets",
        (Key::EmptySubHint, Language::Zh) => "或使用上方按钮添加监控目标",

        // ---------------- Status badges ----------------
        (Key::StatusPending, Language::En) => "Pending",
        (Key::StatusPending, Language::Zh) => "等待检测",
        (Key::StatusScanning, Language::En) => "Scanning",
        (Key::StatusScanning, Language::Zh) => "检测中",
        (Key::StatusIdle, Language::En) => "Not Locked",
        (Key::StatusIdle, Language::Zh) => "未被占用",
        (Key::StatusLockedPrefix, Language::En) => "Locked",
        (Key::StatusLockedPrefix, Language::Zh) => "被占用",
        (Key::StatusFailed, Language::En) => "Scan Failed",
        (Key::StatusFailed, Language::Zh) => "检测失败",
        (Key::StatusAccessDenied, Language::En) => "Access Denied",
        (Key::StatusAccessDenied, Language::Zh) => "权限不足",
        (Key::ProcessUnit, Language::En) => "processes",
        (Key::ProcessUnit, Language::Zh) => "个进程",

        // ---------------- Process row ----------------
        (Key::BtnViewGuide, Language::En) => "View Tips",
        (Key::BtnViewGuide, Language::Zh) => "查看建议",
        (Key::BtnTerminate, Language::En) => "Terminate",
        (Key::BtnTerminate, Language::Zh) => "强制结束",
        (Key::LabelSystemProcess, Language::En) => "System process — handle manually",
        (Key::LabelSystemProcess, Language::Zh) => "系统进程，请手动处理",
        (Key::SubitemPrefix, Language::En) => "· locks",
        (Key::SubitemPrefix, Language::Zh) => "· 占用",
        (Key::SubitemSuffix, Language::En) => "child item(s)",
        (Key::SubitemSuffix, Language::Zh) => "个子项",
        (Key::TooltipTerminate, Language::En) => "Terminate process",
        (Key::TooltipTerminate, Language::Zh) => "结束进程",
        (Key::TooltipRemove, Language::En) => "Remove from list",
        (Key::TooltipRemove, Language::Zh) => "从列表移除",

        // ---------------- Status bar ----------------
        (Key::PrivStandard, Language::En) => "● Standard User",
        (Key::PrivStandard, Language::Zh) => "● 标准用户",
        (Key::PrivAdmin, Language::En) => "● Administrator",
        (Key::PrivAdmin, Language::Zh) => "● 管理员",
        (Key::BtnRestartAdmin, Language::En) => "Restart as Administrator",
        (Key::BtnRestartAdmin, Language::Zh) => "以管理员身份重启",
        (Key::LabelRefresh, Language::En) => "Refresh",
        (Key::LabelRefresh, Language::Zh) => "刷新",
        (Key::ToastClosing, Language::En) => "Close",
        (Key::ToastClosing, Language::Zh) => "关闭",

        // ---------------- Confirm dialogs ----------------
        (Key::DlgTermTitle, Language::En) => "Confirm: Terminate Process",
        (Key::DlgTermTitle, Language::Zh) => "确认强制结束进程",
        (Key::DlgTermMessage, Language::En) => {
            "Are you sure you want to force-terminate this process?"
        }
        (Key::DlgTermMessage, Language::Zh) => "确定要强制结束以下进程吗？",
        (Key::DlgTermWarning, Language::En) => "⚠ Force-terminating may cause data loss!",
        (Key::DlgTermWarning, Language::Zh) => "⚠ 强制结束进程可能导致数据丢失！",
        (Key::DlgTermConfirm, Language::En) => "Terminate",
        (Key::DlgTermConfirm, Language::Zh) => "强制结束",
        (Key::DlgClearTitle, Language::En) => "Confirm: Clear List",
        (Key::DlgClearTitle, Language::Zh) => "确认清空列表",
        (Key::DlgClearMessageFmt, Language::En) => "Will remove all {N} target(s). Continue?",
        (Key::DlgClearMessageFmt, Language::Zh) => "将清空全部 {N} 项监控目标，确定吗？",
        (Key::DlgClearConfirm, Language::En) => "Clear",
        (Key::DlgClearConfirm, Language::Zh) => "确认清空",
        (Key::BtnCancel, Language::En) => "Cancel",
        (Key::BtnCancel, Language::Zh) => "取消",

        // ---------------- Toasts ----------------
        (Key::ToastElevating, Language::En) => "Requesting elevation...",
        (Key::ToastElevating, Language::Zh) => "正在请求提权...",
        (Key::ToastTermInProgressFmt, Language::En) => "Terminating {NAME} (PID {PID})...",
        (Key::ToastTermInProgressFmt, Language::Zh) => "正在结束 {NAME} (PID {PID})...",
        (Key::ToastTermSuccessFmt, Language::En) => "Terminated {NAME} (PID {PID})",
        (Key::ToastTermSuccessFmt, Language::Zh) => "已结束 {NAME} (PID {PID})",
        (Key::ToastTermAlreadyExitedFmt, Language::En) => "{NAME} (PID {PID}) already exited",
        (Key::ToastTermAlreadyExitedFmt, Language::Zh) => "{NAME} (PID {PID}) 已退出",
        (Key::ToastTermAccessDenied, Language::En) => "Access denied. Restart as Administrator.",
        (Key::ToastTermAccessDenied, Language::Zh) => "权限不足，请以管理员身份重启程序",
        (Key::ToastTermTimeout, Language::En) => "Operation timed out. Please retry.",
        (Key::ToastTermTimeout, Language::Zh) => "操作超时，请重试",
        (Key::ToastTermSystemProtected, Language::En) => "System process cannot be terminated",
        (Key::ToastTermSystemProtected, Language::Zh) => "系统进程不可结束",
        (Key::ToastTermStalePidFmt, Language::En) => "PID {PID} has been reused; operation skipped",
        (Key::ToastTermStalePidFmt, Language::Zh) => "PID {PID} 已复用，已忽略本次操作",

        // ---------------- Misc ----------------
        (Key::MsgUnsupportedOs, Language::En) => {
            "Unsupported Windows version (Windows 10 1809+ required). Some features may not work."
        }
        (Key::MsgUnsupportedOs, Language::Zh) => {
            "当前系统版本不受支持（需要 Windows 10 1809 或更高版本），部分功能可能无法正常工作"
        }

        // ---------------- System guide (multi-line) ----------------
        (Key::SystemGuideText, Language::En) => {
            "\
System Process — Recommended Actions:\n\
\n\
1. Verify the process via Task Manager → Services tab\n\
2. For svchost.exe, inspect the hosted services\n\
3. Run sfc /scannow to check system file integrity\n\
4. If held by Windows Update, wait for it to finish\n\
5. A reboot usually releases system process locks\n\
\n\
⚠ Do NOT force-terminate system processes — system instability or BSOD may occur!"
        }
        (Key::SystemGuideText, Language::Zh) => {
            "\
系统进程处置建议：\n\
\n\
1. 通过任务管理器 → 服务页面确认该进程的功能\n\
2. 若是 svchost.exe，可查看其托管的具体服务\n\
3. 使用 sfc /scannow 检查系统文件完整性\n\
4. 若文件被 Windows Update 占用，等待更新完成后重试\n\
5. 重启计算机通常可以解除系统进程的占用\n\
\n\
⚠ 切勿强制结束系统进程，否则可能导致系统不稳定或蓝屏！"
        }
    }
}

/// 简易格式化：把模板中的 `{NAME}` / `{PID}` / `{N}` 占位符替换为给定值。
///
/// 仅支持 `{NAME}` / `{PID}` / `{N}` 三个键，未列出的占位符保持原样。
pub fn format_pid(template: &str, name: &str, pid: u32) -> String {
    template
        .replace("{NAME}", name)
        .replace("{PID}", &pid.to_string())
}

pub fn format_n(template: &str, n: usize) -> String {
    template.replace("{N}", &n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_chinese() {
        // 注意：测试可能与其他线程共享 CURRENT_LANG，先复位到默认中文
        set_language(Language::Zh);
        assert_eq!(current_language(), Language::Zh);
        assert_eq!(t(Key::BtnAddFile), "+ 添加文件");
    }

    #[test]
    fn switch_to_english() {
        set_language(Language::En);
        assert_eq!(t(Key::BtnAddFile), "+ Add File");
        // 复位为默认（中文），避免污染其他测试
        set_language(Language::Zh);
    }

    #[test]
    fn language_default_trait_is_chinese() {
        assert_eq!(Language::default(), Language::Zh);
    }

    #[test]
    fn format_helpers_replace_placeholders() {
        let s = format_pid("Terminating {NAME} (PID {PID})", "notepad.exe", 1234);
        assert_eq!(s, "Terminating notepad.exe (PID 1234)");
        let s2 = format_n("Will remove {N} items", 5);
        assert_eq!(s2, "Will remove 5 items");
    }
}
