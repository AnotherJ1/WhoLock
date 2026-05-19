# 🔍 File Lock Inspector

A modern Windows desktop tool to find out **which processes are holding a lock on your files or folders** — and force-close the offenders.

> Tired of `The action can't be completed because the file is open in another program`? This tool tells you exactly which program, and lets you terminate it (when safe to do so).

[简体中文](#中文版本) · English

---

## ✨ Features

- 🎯 **Pinpoint the locker** — uses Windows Restart Manager API to find every process holding the file
- 🖱️ **Drag-and-drop support** — drop files or folders directly into the window
- 🔄 **Live monitoring** — auto-refresh at 1s / 2s / 5s / 10s intervals
- ⚡ **One-click force terminate** — kill non-system processes with a confirmation dialog
- 🛡️ **System process protection** — three-layer classifier (PID + blacklist + SID/path) prevents accidental termination of OS processes
- 🚀 **Privilege escalation** — re-launch as Administrator when extra access is needed
- 🌐 **i18n built-in** — English (default) and 简体中文, switchable in-app
- 🎨 **Modern dark UI** — built with [egui](https://github.com/emilk/egui), beautiful out of the box
- 📝 **Audit logs** — daily-rotated logs in `%LOCALAPPDATA%`

---

## 📸 Screenshot

```
┌──────────────────────────────────────────────────────────────────────┐
│ 🔍 File Lock Inspector  [+ Add File] [+ Add Folder] [Clear] [中文]   │
├──────────────────────────────────────────────────────────────────────┤
│ ● Locked · 2 processes  C:\path\to\report.docx              ✕       │
│   ┌ PID 12345  WINWORD.EXE  ALICE\alice           [Terminate]       │
│   │ 📄 C:\Program Files\Microsoft Office\WINWORD.EXE                 │
│   └ PID 67890  searchindexer.exe  System process — handle manually  │
│                                                                      │
│ ● Not Locked  D:\readme.md                                  ✕       │
├──────────────────────────────────────────────────────────────────────┤
│ ● Standard User  [Restart as Administrator]   Refresh: 1s [2s] 5s   │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### Download

Grab the latest `file-lock-inspector.exe` from the [Releases](#) page (single-file, no installer).

### Run

```powershell
.\file-lock-inspector.exe
```

1. Drag a file or folder onto the window — or click **+ Add File** / **+ Add Folder**
2. The tool scans every 2 seconds and lists every process holding a lock
3. Click **Terminate** to force-close a non-system process (with confirmation)
4. If a banner says "Access Denied", click **Restart as Administrator**

---

## 📋 System Requirements

| | |
|---|---|
| **OS** | Windows 10 1809 (build 17763) or later, **64-bit only** |
| **Architecture** | `x86_64` (AMD64) |
| **Runtime** | None — fully self-contained, ~5 MB |
| **Permissions** | Standard user for basic scanning; Administrator for system-locked files |

---

## 🌐 Language

Default language is **English**. Click the **中文** button in the toolbar to switch to Simplified Chinese (and vice-versa). Your choice is saved to `config.json` and remembered across launches.

---

## 🔐 Privilege Escalation

When a file is locked by a privileged process (e.g. a Windows service running as `SYSTEM`), the scanner will report `Access Denied`. Click **Restart as Administrator** in the bottom-left status bar — Windows will show a UAC prompt; accept it and the tool re-launches with full access. Declining UAC keeps the current session running unchanged.

---

## 📂 File Locations

| Path | Purpose |
|---|---|
| `%LOCALAPPDATA%\FileLockInspector\config.json` | UI preferences (language, refresh interval, window size) |
| `%LOCALAPPDATA%\FileLockInspector\logs\fli.log.YYYY-MM-DD` | Daily-rotated logs (kept for 30 days, max 10 MB/day) |

The toolbar **Open Log Folder** button opens the log directory in Explorer.

---

## 🏗️ Architecture

```
┌─────────────────────┐     ┌──────────────────────┐
│   UI Layer (egui)   │◄───►│   AppState (Mutex)   │
│  toolbar / dialogs  │     │  targets, settings   │
└──────────┬──────────┘     └──────────────────────┘
           │ MonitorCmd                  ▲
           ▼                             │ ScanEvent
┌─────────────────────┐     ┌──────────────────────┐
│   MonitorEngine     │────►│   detector::scan     │
│  scheduler + 4×pool │     │  Restart Manager API │
└─────────────────────┘     └──────────┬───────────┘
                                       │
                          ┌────────────┴────────────┐
                          ▼                         ▼
                ┌──────────────────┐    ┌─────────────────────┐
                │  process_info    │    │  sys_classifier     │
                │  PID → name/sid  │    │  3-layer blacklist  │
                └──────────────────┘    └─────────────────────┘
```

| Module | Responsibility |
|---|---|
| `state` | `AppState`, `TargetItem`, `UiCmd` — shared application state |
| `detector` | Restart Manager wrapper, `ProcessRecord` discovery, merge logic |
| `sys_classifier` | Three-layer system-process classifier (PID 0/4 + blacklist) ∪ (SID + System32 path) ∪ (info-missing fallback) |
| `monitor` | Background polling engine with `Clock` abstraction (`SystemClock` + `FakeClock` for tests) |
| `process_info` | `OpenProcess` + `QueryFullProcessImageNameW` + token SID lookup |
| `terminator` | `force_terminate` with PID-reuse defense + 5s timeout wrapper |
| `elevation` | `is_elevated()`, `restart_as_admin()` via `ShellExecuteExW("runas")` |
| `i18n` | Lightweight key-based translation system, English + Simplified Chinese |
| `ui` | egui components: `target_list`, `process_row`, `status_bar`, `dialogs`, `dropping` |
| `error` | Typed error hierarchy — `AppError`, `TerminateError`, `ScanFailure`, `RmError` |

---

## 🧪 Testing

```powershell
# Unit + property tests (82 tests)
cargo test

# Property-Based Tests use proptest (≥ 100 cases each)
cargo test prop_

# Win32 integration tests (require real PowerShell + tempdir)
cargo test -- --ignored
```

### Coverage Highlights

14 formal **Correctness Properties** are validated by `proptest`:

| # | Property | Module |
|---|---|---|
| 1 | `try_add_target` dedup + existence semantics | `state` |
| 2 | `enumerate_direct_children` only depth-1 | `detector::enumerate` |
| 3 | `merge_process_records` invariants | `detector` |
| 4 | Polling frequency `\|n − T/interval\| ≤ 1` | `monitor::scheduler` |
| 5 | `apply_scan_event` reflects latest result | `state` |
| 6 | No concurrent scan for same target | `monitor::scheduler` |
| 7 | `force_terminate` error-code mapping | `terminator` |
| 8 | `is_system_process` three-layer classifier | `sys_classifier` |
| 9 | System process UI + termination protection | `ui::process_row` + `terminator` |
| 10 | `RemoveTarget` / `ClearAll` idempotence | `state` |
| 11 | Empty target list pauses polling | `monitor::scheduler` |
| 12 | Cancel keeps state identical | `state` |
| 13 | Batch-add toast aggregation | `state` |
| 14 | `SetInterval` does not interrupt in-flight scan | `monitor::scheduler` |

---

## 🔨 Building from Source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, **1.78+**)
- Visual Studio 2022 Build Tools (or Visual Studio with C++ workload)
- Target: `rustup target add x86_64-pc-windows-msvc`

### Debug Build

```powershell
cargo build
cargo run
```

### Release Build (recommended for distribution)

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

Output: `target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe`

### Verify the Binary

```powershell
# 1. Single executable, no DLLs required
ls target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe

# 2. Manifest embedded (asInvoker, PerMonitorV2 DPI, supportedOS GUIDs)
mt.exe -inputresource:"target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe;#1" -out:manifest.xml
cat manifest.xml

# 3. Symbols stripped (release profile)
# 4. Standalone launch from any directory
```

---

## 🐛 Troubleshooting

**Q: The window text shows as boxes / squares.**
A: The app loads `msyh.ttc` / `simsun.ttc` from `C:\Windows\Fonts` at startup. If those are missing or corrupted, install the **Microsoft YaHei** font from Windows Update.

**Q: I see `Access Denied` but the process is mine.**
A: Some processes (e.g. ones spawned by services) require Administrator to inspect. Click **Restart as Administrator**.

**Q: Force-terminate fails with "Operation timed out".**
A: An anti-virus or kernel hook may be blocking `TerminateProcess`. Check the log file for the underlying error code, or temporarily disable the AV and retry.

**Q: Where are the logs?**
A: `%LOCALAPPDATA%\FileLockInspector\logs\` — click **Open Log Folder** in the toolbar.

**Q: How can I reset the configuration?**
A: Delete `%LOCALAPPDATA%\FileLockInspector\config.json` and relaunch.

---

## 🤝 Contributing

This project is built around the spec in `.kiro/specs/file-lock-inspector/` (requirements, design, tasks). Pull requests should:

1. Pass `cargo test` (82 tests)
2. Pass `cargo clippy --all-targets -- -D warnings`
3. Pass `cargo fmt --check`
4. Add new translations to `src/i18n.rs` for both `Language::En` and `Language::Zh`
5. Update relevant `Property N` PBT if behavior changes

---

## 📜 License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.

---

## 中文版本

**文件占用查询（File Lock Inspector）** 是一款 Windows 桌面工具，用于排查"文件正在被另一程序使用"问题：

- 列出每个文件/文件夹被哪些进程占用
- 一键强制结束非系统进程（带二次确认）
- 三层判定保护系统进程，避免误杀
- 拖放添加 / 实时刷新（1/2/5/10 秒可选）
- 标准用户权限不够时一键以管理员身份重启
- 日志按日轮转，保留 30 天
- 内置中英文界面切换（默认英文）

### 系统要求

- Windows 10 1809 (build 17763) 及以上，**仅 64 位**
- 单一 exe 文件（约 5MB），无外部依赖

### 快速开始

下载 `file-lock-inspector.exe` 双击运行 → 拖入文件或文件夹 → 点击 **强制结束** 释放锁 → 完成。

### 切换中文界面

启动后点击工具栏右上角 **中文** 按钮即可切换；选择会持久化到 `%LOCALAPPDATA%\FileLockInspector\config.json`。

### 从源码构建

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

更多详情参见上方英文章节或 `.kiro/specs/file-lock-inspector/` 设计文档。
