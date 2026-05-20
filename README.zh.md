# 🔍 文件占用查询

一款 Windows 桌面工具，用于找出**哪些进程占用了你的文件或文件夹**，并可强制结束占用进程。

> 受够了 `无法完成操作，因为文件已在另一程序中打开`？这个工具会告诉你具体是哪个程序，并允许你在安全范围内结束它。

---

## ✨ 功能特点

- 🎯 **精确定位占用进程** — 基于 Windows Restart Manager API，找出每个持有文件句柄的进程
- 🖱️ **拖放支持** — 直接将文件或文件夹拖入窗口
- 🔄 **实时监控** — 支持 1s / 2s / 5s / 10s 自动刷新
- ⚡ **一键强制结束** — 带确认对话框，安全结束非系统进程
- 🛡️ **系统进程保护** — 三层分类器（PID + 黑名单 + SID/路径），防止误杀 OS 进程
- 🚀 **权限提升** — 需要时一键以管理员身份重启
- 🌐 **内置国际化** — 英文（默认）和简体中文，应用内可切换
- 🎨 **现代深色 UI** — 基于 [egui](https://github.com/emilk/egui) 构建，开箱即美
- 📝 **审计日志** — 按日轮转，保存在 `%LOCALAPPDATA%`

---

## 📸 界面预览

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

## 🚀 快速开始

### 下载

从 [Releases](https://github.com/AnotherJ1/WhoLock/releases) 页面获取最新 `file-lock-inspector.exe`（单文件，无需安装）。

### 运行

```powershell
.\file-lock-inspector.exe
```

1. 将文件或文件夹拖入窗口，或点击 **+ Add File** / **+ Add Folder**
2. 工具每 2 秒扫描一次，列出所有持有文件锁的进程
3. 点击 **Terminate** 强制结束非系统进程（需确认）
4. 如果提示 "Access Denied"，点击 **Restart as Administrator**

---

## 📋 系统要求

| | |
|---|---|
| **操作系统** | Windows 10 1809 (build 17763) 或更高版本，**仅 64 位** |
| **架构** | `x86_64` (AMD64) |
| **运行时** | 无需安装 — 完全自包含，约 5 MB |
| **权限** | 标准用户可基本扫描；管理员权限可处理系统级锁定文件 |

---

## 🌐 语言

默认语言为 **English**。点击工具栏中的 **中文** 按钮可切换为简体中文（反之亦然）。您的选择会保存到 `config.json`，重启后自动记忆。

---

## 🔐 权限提升

当文件被特权进程（例如以 `SYSTEM` 身份运行的 Windows 服务）锁定时，扫描器会报告 `Access Denied`。点击左下角状态栏的 **Restart as Administrator** — Windows 会弹出 UAC 提示；接受后工具将以完全权限重新启动。拒绝 UAC 则当前会话保持不变。

---

## 📂 文件位置

| 路径 | 用途 |
|---|---|
| `%LOCALAPPDATA%\FileLockInspector\config.json` | UI 偏好设置（语言、刷新间隔、窗口大小） |
| `%LOCALAPPDATA%\FileLockInspector\logs\fli.log.YYYY-MM-DD` | 按日轮转的日志（保留 30 天，每天最大 10 MB） |

点击工具栏中的 **Open Log Folder** 可在资源管理器中打开日志目录。

---

## 🏗️ 架构

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

| 模块 | 职责 |
|---|---|
| `state` | `AppState`、`TargetItem`、`UiCmd` — 共享应用状态 |
| `detector` | Restart Manager 封装、`ProcessRecord` 发现、合并逻辑 |
| `sys_classifier` | 三层系统进程分类器：(PID 0/4 + 黑名单) ∪ (SID + System32 路径) ∪ (信息缺失时的保守策略) |
| `monitor` | 后台轮询引擎，带 `Clock` 抽象（`SystemClock` + `FakeClock`，用于测试） |
| `process_info` | `OpenProcess` + `QueryFullProcessImageNameW` + token SID 查询 |
| `terminator` | `force_terminate` 带 PID 复用防御 + 5 秒超时包装 |
| `elevation` | `is_elevated()`、`restart_as_admin()` 通过 `ShellExecuteExW("runas")` |
| `i18n` | 轻量级键值翻译系统，支持英文和简体中文 |
| `ui` | egui 组件：`target_list`、`process_row`、`status_bar`、`dialogs`、`dropping` |
| `error` | 类型化错误层次 — `AppError`、`TerminateError`、`ScanFailure`、`RmError` |

---

## 🧪 测试

```powershell
# 单元测试 + 属性测试（86 个测试）
cargo test

# 属性测试使用 proptest（每项 ≥ 100 个用例）
cargo test prop_

# Win32 集成测试（需要真实 PowerShell + 临时目录）
cargo test -- --ignored
```

### 测试覆盖亮点

14 项 **正确性属性** 通过 `proptest` 验证：

| # | 属性 | 模块 |
|---|---|---|
| 1 | `try_add_target` 去重 + 存在性语义 | `state` |
| 2 | `enumerate_direct_children` 仅返回深度-1 | `detector::enumerate` |
| 3 | `merge_process_records` 不变性 | `detector` |
| 4 | 轮询频率 `\|n − T/interval\| ≤ 1` | `monitor::scheduler` |
| 5 | `apply_scan_event` 反映最新结果 | `state` |
| 6 | 同一目标不会并发扫描 | `monitor::scheduler` |
| 7 | `force_terminate` 错误码映射 | `terminator` |
| 8 | `is_system_process` 三层分类器 | `sys_classifier` |
| 9 | 系统进程 UI + 终止保护 | `ui::process_row` + `terminator` |
| 10 | `RemoveTarget` / `ClearAll` 幂等性 | `state` |
| 11 | 空目标列表暂停轮询 | `monitor::scheduler` |
| 12 | 取消操作保持状态不变 | `state` |
| 13 | 批量添加的 Toast 聚合 | `state` |
| 14 | `SetInterval` 不影响正在进行的扫描 | `monitor::scheduler` |

---

## 🔨 从源码构建

### 前置要求

- [Rust 工具链](https://rustup.rs/)（稳定版，**1.78+**）
- Visual Studio 2022 Build Tools（或安装了 C++ 工作负载的 Visual Studio）
- 目标平台：`rustup target add x86_64-pc-windows-msvc`

### 调试构建

```powershell
cargo build
cargo run
```

### 发布构建（推荐用于分发）

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

输出：`target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe`

### 验证二进制文件

```powershell
# 1. 单文件，无需任何 DLL
ls target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe

# 2. 已嵌入清单（asInvoker、PerMonitorV2 DPI、supportedOS GUIDs）
mt.exe -inputresource:"target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe;#1" -out:manifest.xml
cat manifest.xml

# 3. 符号已剥离（release profile）
# 4. 可从任意目录独立启动
```

---

## 🐛 故障排除

**问：窗口文字显示为方框/方块。**
答：应用启动时会从 `C:\Windows\Fonts` 加载 `msyh.ttc` / `simsun.ttc`。如果这些字体缺失或损坏，请从 Windows Update 安装 **Microsoft YaHei** 字体。

**问：看到 `Access Denied`，但进程是我的。**
答：某些进程（例如由服务启动的进程）需要管理员权限才能检查。请点击 **Restart as Administrator**。

**问：强制结束失败，提示 "Operation timed out"。**
答：杀毒软件或内核钩子可能正在拦截 `TerminateProcess`。请查看日志文件了解底层错误码，或暂时禁用杀毒软件后重试。

**问：日志在哪里？**
答：`%LOCALAPPDATA%\FileLockInspector\logs\` — 点击工具栏中的 **Open Log Folder** 即可打开。

**问：如何重置配置？**
答：删除 `%LOCALAPPDATA%\FileLockInspector\config.json` 然后重新启动应用。

---

## 🤝 贡献指南

本项目围绕 `.kiro/specs/file-lock-inspector/` 中的规范（需求、设计、任务）构建。Pull Request 应符合以下要求：

1. 通过 `cargo test`（86 个测试）
2. 通过 `cargo clippy --all-targets -- -D warnings`
3. 通过 `cargo fmt --check`
4. 新翻译添加到 `src/i18n.rs` 的 `Language::En` 和 `Language::Zh`
5. 如果行为发生变化，更新相应的 `Property N` 属性测试

---

## 📜 许可证

根据 [Apache License 2.0](LICENSE-APACHE) 或 [MIT License](LICENSE-MIT)（您可任选其一）授权。
