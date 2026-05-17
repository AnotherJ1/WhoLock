# 需求文档（Requirements Document）

## Introduction

File_Lock_Inspector 是一款仅运行于 Windows 桌面的工具软件，用于帮助用户排查指定文件或文件夹被哪些进程占用，并在安全前提下提供"强制结束占用进程"的能力。用户在删除、移动、重命名文件时常因"文件正在被另一程序使用"而受阻，本工具通过列出占用句柄的进程、持续刷新占用状态、保护系统关键进程，帮助用户快速定位并解除占用。

## Glossary

- **File_Lock_Inspector**：本应用程序整体，运行于 Windows 桌面环境。
- **Target_Item**：用户提交给程序进行检测的对象，可以是单个文件或单个文件夹。
- **Target_List**：当前正在被监控的全部 Target_Item 集合。
- **File_Selector**：负责接收用户文件/文件夹选择输入的子模块（含对话框选择与拖放接收）。
- **Lock_Detector**：负责查询并解析当前 Target_Item 被哪些进程占用的检测引擎。
- **Locking_Process**：持有 Target_Item 句柄或对其加锁的 Windows 进程。
- **Process_Record**：单条占用记录，包含进程标识与占用上下文信息。
- **Monitor_Engine**：以固定周期重新触发 Lock_Detector 的后台轮询模块。
- **System_Process**：由 Windows 操作系统核心组件持有的进程，对其结束会导致系统不稳定或蓝屏。判定规则在需求中显式定义。
- **Force_Terminate_Action**：调用 Windows API 强制结束目标进程的操作。
- **Polling_Interval**：Monitor_Engine 两次连续检测之间的时间间隔，单位为毫秒。
- **Privilege_Level**：当前应用进程拥有的 Windows 访问令牌级别，区分"标准用户"与"管理员"。
- **UI_Layer**：负责渲染界面与接收用户操作的前端模块。
- **Restart_Manager_API**：Windows 系统提供的一组 API（rstrtmgr.dll，函数族 RmStartSession / RmRegisterResources / RmGetList / RmEndSession），用于查询哪些进程对指定文件路径持有占用，本应用作为 Lock_Detector 的底层实现机制。

## Requirements

### Requirement 1：文件与文件夹的选择输入

**User Story:** 作为用户，我希望通过对话框或拖放方式将文件或文件夹添加到检测列表，以便快速指定需要排查占用的目标。

#### Acceptance Criteria

1. WHEN 用户点击"添加文件"按钮，THE File_Selector SHALL 打开 Windows 标准文件选择对话框，并允许一次选择多个文件
2. WHEN 用户点击"添加文件夹"按钮，THE File_Selector SHALL 打开 Windows 标准文件夹选择对话框；由于 Windows IFileOpenDialog 单次仅支持选择一个文件夹，THE File_Selector SHALL 允许用户通过连续多次点击该按钮或使用拖放方式累计添加多个文件夹
3. WHEN 用户将一个或多个文件或文件夹拖入 File_Lock_Inspector 主窗口，THE File_Selector SHALL 将所有被拖入的项作为 Target_Item 加入 Target_List
4. WHEN File_Selector 接收到一个新的 Target_Item，THE File_Selector SHALL 在添加到 Target_List 之前校验其在文件系统中存在
5. IF 一个被提交的路径在文件系统中不存在，THEN THE File_Selector SHALL 拒绝该项并将"路径不存在"原因纳入本次批量结果中
6. IF 一个被提交的路径已经存在于 Target_List 中，THEN THE File_Selector SHALL 跳过该项不重复添加，并将"已存在"原因纳入本次批量结果中
7. WHERE Target_Item 为文件夹，THE Lock_Detector SHALL 同时检测该文件夹自身以及其直接子文件与一级子文件夹的占用情况，且不递归进入更深层级
8. THE File_Lock_Inspector SHALL 允许 Target_List 同时包含至少 50 个 Target_Item
9. WHEN 用户单次操作（拖放、文件对话框、文件夹对话框）一次提交了多个路径，THE UI_Layer SHALL 以批量摘要的形式聚合提示结果，格式为"成功添加 X 项 / 跳过 Y 项已存在 / 拒绝 Z 项不存在"，且不为同一次批量操作的每条路径分别弹出独立 Toast

### Requirement 2：占用进程检测

**User Story:** 作为用户，我希望软件自动检测每个目标对象当前被哪些进程占用，以便明确解除占用的对象。

#### Acceptance Criteria

1. WHEN 一个 Target_Item 被加入 Target_List，THE Lock_Detector SHALL 在 1000 毫秒内对该项执行首次占用检测
2. WHEN Lock_Detector 完成对一个 Target_Item 的检测，THE Lock_Detector SHALL 输出零个或多个 Process_Record，每条 Process_Record 至少包含：PID、进程名、可执行文件路径（无权限时可为空）、进程启动时间、应用类型（Application / Service / Console / Critical / Unknown，由 Restart_Manager_API 的 strAppType 字段直接映射）
3. WHEN Lock_Detector 检测到同一进程在同一 Target_Item 下被注册的多个资源路径上均出现，THE Lock_Detector SHALL 在 UI_Layer 上将其合并为单条 Process_Record，并在该记录中标注"占用 N 个子项"，其中 N 为该 PID 在本次扫描中出现的资源条目数
4. WHERE Target_Item 为文件夹，THE Lock_Detector SHALL 在每条 Process_Record 中至少标注一个具体被占用的子路径；当占用多个子路径时仅展示其中一个并以"占用 N 个子项"提示总数
5. IF Lock_Detector 在检测某个 Target_Item 时遇到访问拒绝错误，THEN THE Lock_Detector SHALL 在 UI_Layer 上为该 Target_Item 显示状态"权限不足，需要管理员"，且不将其视为"未被占用"
6. IF Lock_Detector 在检测过程中发生未预期异常，THEN THE Lock_Detector SHALL 记录该异常到本地日志，并在 UI_Layer 上为该 Target_Item 显示状态"检测失败"
7. THE File_Lock_Inspector SHALL 不向用户承诺提供文件级"读/写/删除"等访问模式信息，因为 Restart_Manager_API 不暴露此类细节

### Requirement 3：占用状态的持续监控与刷新

**User Story:** 作为用户，我希望软件持续监控占用状态，当占用解除后能自动清空该项记录，以便我无需手动刷新。

#### Acceptance Criteria

1. WHILE Target_List 非空，THE Monitor_Engine SHALL 按照 Polling_Interval 周期性地对 Target_List 中的每个 Target_Item 重新执行 Lock_Detector
2. THE File_Lock_Inspector SHALL 将默认 Polling_Interval 设置为 2000 毫秒
3. THE File_Lock_Inspector SHALL 允许用户在 UI_Layer 上将 Polling_Interval 调整为 1000、2000、5000、10000 毫秒中的一个
4. WHEN 用户切换 Polling_Interval，THE Monitor_Engine SHALL 在当前正在进行的轮询完成后，按新的间隔安排下一次轮询，且不中断当前轮询
5. WHEN Monitor_Engine 在某次检测中发现一个 Target_Item 的 Process_Record 列表由非空变为空，THE UI_Layer SHALL 在 500 毫秒内将该 Target_Item 的占用记录清空，并将其状态标记为"未被占用"
6. WHEN Monitor_Engine 在某次检测中发现一个 Target_Item 的 Process_Record 列表发生增删，THE UI_Layer SHALL 在 500 毫秒内更新该 Target_Item 显示的 Process_Record 列表
7. WHILE 一次检测正在进行，THE Monitor_Engine SHALL 不启动针对同一 Target_Item 的并发检测
8. IF 某次轮询的总耗时超过 Polling_Interval，THEN THE Monitor_Engine SHALL 等待当前轮询完成后再开始下一轮，且不累积排队
9. WHEN 用户从 Target_List 中移除某个 Target_Item，THE Monitor_Engine SHALL 在下一轮询前停止对该项的检测并删除其全部 Process_Record

### Requirement 4：强制结束占用进程

**User Story:** 作为用户，我希望对非系统的占用进程一键强制结束，以便快速解除占用。

#### Acceptance Criteria

1. WHERE 一条 Process_Record 对应的 Locking_Process 不是 System_Process，THE UI_Layer SHALL 在该 Process_Record 行上显示"强制结束"按钮
2. WHEN 用户点击某条 Process_Record 上的"强制结束"按钮，THE UI_Layer SHALL 弹出二次确认对话框，包含进程名与 PID
3. WHEN 用户在二次确认对话框中确认，THE File_Lock_Inspector SHALL 对该 Locking_Process 执行 Force_Terminate_Action，并在执行的同时通过 UI_Layer 显示"正在结束进程..."临时状态
4. WHEN Force_Terminate_Action 调用未在 5000 毫秒内返回，THE File_Lock_Inspector SHALL 中止等待并视为失败，UI_Layer 显示"操作超时，请重试"提示
5. WHEN Force_Terminate_Action 成功结束目标进程，THE Monitor_Engine SHALL 在 1000 毫秒内触发一次额外的占用检测以刷新 UI_Layer
6. IF Force_Terminate_Action 因权限不足失败，THEN THE UI_Layer SHALL 提示"权限不足，请以管理员身份重启程序"，并提供"以管理员身份重启"按钮
7. IF Force_Terminate_Action 因目标进程已不存在而失败，THEN THE File_Lock_Inspector SHALL 视为成功，并触发一次占用刷新
8. IF Force_Terminate_Action 因其他原因失败，THEN THE UI_Layer SHALL 显示具体的 Windows 错误码与可读描述
9. WHEN 用户在二次确认对话框中取消，THE File_Lock_Inspector SHALL 不执行 Force_Terminate_Action 且不改变任何状态

### Requirement 5：系统进程保护

**User Story:** 作为用户，我希望软件主动识别系统进程并禁止误结束，以避免操作系统崩溃。

#### Acceptance Criteria

1. THE File_Lock_Inspector SHALL 将下列进程判定为 System_Process：PID 为 0 的 System Idle Process、PID 为 4 的 System、smss.exe、csrss.exe、wininit.exe、winlogon.exe、services.exe、lsass.exe、lsm.exe（注：Win10/11 上 lsm 已并入 services，保留作为防御性匹配）、svchost.exe、Registry、MemCompression
2. THE File_Lock_Inspector SHALL 将运行账户为 NT AUTHORITY\\SYSTEM、NT AUTHORITY\\LOCAL SERVICE、NT AUTHORITY\\NETWORK SERVICE 中任意一项，且可执行文件位于 %WINDIR%\\System32 或 %WINDIR%\\SysWOW64 目录下的进程，判定为 System_Process
3. WHERE 一条 Process_Record 对应的 Locking_Process 是 System_Process，THE UI_Layer SHALL 在该 Process_Record 行上隐藏"强制结束"按钮，并显示文案"系统进程，请手动处理"
4. WHERE 一条 Process_Record 对应的 Locking_Process 是 System_Process，THE UI_Layer SHALL 提供一个"查看处理建议"链接，点击后展示固定的处置指南文本
5. IF 用户通过任何方式尝试对 System_Process 触发 Force_Terminate_Action，THEN THE File_Lock_Inspector SHALL 拒绝执行并在 UI_Layer 上显示"系统进程不可结束"

### Requirement 6：权限与管理员重启

**User Story:** 作为用户，我希望在权限不足时能一键以管理员身份重启程序，以便检测和结束更多进程。

#### Acceptance Criteria

1. WHEN File_Lock_Inspector 启动，THE File_Lock_Inspector SHALL 检测当前 Privilege_Level 并在 UI_Layer 状态栏显示"标准用户"或"管理员"
2. WHERE Privilege_Level 为"标准用户"，THE UI_Layer SHALL 在状态栏显示"以管理员身份重启"按钮
3. WHEN 用户点击"以管理员身份重启"按钮，THE File_Lock_Inspector SHALL 通过 ShellExecute 的 runas 动词重新启动自身，并在新进程启动后退出当前进程
4. IF 用户在 UAC 提示中拒绝提权，THEN THE File_Lock_Inspector SHALL 保持当前进程继续运行且不显示错误
5. WHERE Privilege_Level 为"管理员"，THE UI_Layer SHALL 隐藏"以管理员身份重启"按钮

### Requirement 7：列表管理与批量操作

**User Story:** 作为用户，我希望能管理检测列表中的项目，以便清理不再关注的目标。

#### Acceptance Criteria

1. WHEN 用户在 UI_Layer 上对某一 Target_Item 点击"移除"，THE File_Lock_Inspector SHALL 立即将该项从 Target_List 中删除
2. WHEN 用户点击"清空列表"按钮，THE UI_Layer SHALL 弹出二次确认对话框
3. WHEN 用户在清空列表二次确认中确认，THE File_Lock_Inspector SHALL 移除 Target_List 中的全部 Target_Item
4. WHILE Target_List 为空，THE Monitor_Engine SHALL 暂停所有轮询活动
5. THE UI_Layer SHALL 为每个 Target_Item 显示当前状态之一：等待检测、检测中、未被占用、被占用（N 个进程）、检测失败、权限不足

### Requirement 8：日志与错误处理

**User Story:** 作为用户，当软件出错时我希望有可查询的日志，以便排查问题或反馈给开发者。

#### Acceptance Criteria

1. THE File_Lock_Inspector SHALL 将所有未捕获异常以及 Lock_Detector、Force_Terminate_Action 的失败事件写入日志文件
2. THE File_Lock_Inspector SHALL 将日志文件存放在 %LOCALAPPDATA%\\FileLockInspector\\logs 目录下，按日期轮转，单文件不超过 10 MB
3. THE UI_Layer SHALL 提供"打开日志目录"入口，点击后调用资源管理器打开日志目录
4. IF 日志写入因磁盘空间不足失败，THEN THE File_Lock_Inspector SHALL 在 UI_Layer 上显示一次性提示，并在内存中继续运行不崩溃

### Requirement 9：技术约束（非功能性需求）

**User Story:** 作为开发与维护者，我希望明确实现技术栈与底层 API 选型，以便保证可移植性与长期维护成本可控。

#### Acceptance Criteria

1. THE File_Lock_Inspector SHALL 使用 Rust 语言实现，并以 egui/eframe 作为 UI_Layer 的渲染框架，构建为单一 Windows 原生可执行文件
2. THE Lock_Detector SHALL 通过 Restart_Manager_API（RmStartSession、RmRegisterResources、RmGetList、RmEndSession）实现占用检测，且不依赖 SysInternals handle.exe 等外部可执行文件
3. THE File_Lock_Inspector SHALL 通过 windows-rs crate 调用全部 Win32 API（含 Restart Manager、进程枚举、令牌查询、ShellExecuteW、TerminateProcess 等），且不通过 NtQuerySystemInformation 等未文档化接口
4. THE File_Lock_Inspector SHALL 仅支持 Windows 10（1809 及以上）与 Windows 11，64 位平台，且不提供 32 位发布物
5. WHERE 程序运行的 Windows 版本低于受支持版本，THE File_Lock_Inspector SHALL 在 UI_Layer 上显示"当前系统版本不受支持"提示，但不阻止程序继续运行
