# Manual Test Checklist — File Lock Inspector (WhoLock)

Use this checklist for manual QA before each release. Mark each item ✅ pass, ❌ fail, or ⚠️ partial.

---

## 0. Release Build Artifact Verification

Run before any functional testing to confirm the binary is valid.

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 0.1 | Run `cargo build --release --target x86_64-pc-windows-msvc` | Build completes with exit code 0; no errors | |
| 0.2 | Confirm single output file at `target\x86_64-pc-windows-msvc\release\file-lock-inspector.exe` | Exactly one `.exe`; no additional DLLs required | |
| 0.3 | Check binary size: `(Get-Item file-lock-inspector.exe).Length` | Size < 20 MB (expected ~115 KB stripped) | |
| 0.4 | Verify embedded manifest: `mt.exe -inputresource:"file-lock-inspector.exe;1" -out:"manifest.xml"` | `manifest.xml` contains `asInvoker`, `PerMonitorV2`, and `supportedOS` Win10/Win11 GUIDs | |
| 0.5 | Confirm symbols stripped: binary size is consistent with `strip = "symbols"` profile | No `.pdb` required at runtime; binary runs standalone | |
| 0.6 | Double-click `file-lock-inspector.exe` on a clean Windows 10 1809+ machine | Application window opens without UAC prompt (asInvoker) | |

---

## 1. Drag-and-Drop File / Folder

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 1.1 | Drag a single file from Explorer onto the WhoLock window | File appears in the target list with status `Pending`, then transitions to `Idle` or `Locked` after first scan | |
| 1.2 | Drag a single folder onto the window | Folder appears; direct children are enumerated for lock detection | |
| 1.3 | Drag multiple files at once | All files added in a single batch; toast shows "成功添加 N 项 / …" summary | |
| 1.4 | Drag a file that is already in the list | Toast shows duplicate-skip count; target list unchanged | |
| 1.5 | Drag a path that no longer exists on disk | Toast shows rejected-missing count; target not added | |

---

## 2. Add File / Folder via Dialog

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 2.1 | Click **Add File**, select a file in the Open dialog, confirm | File added to target list | |
| 2.2 | Click **Add Folder**, select a folder, confirm | Folder added to target list | |
| 2.3 | Open dialog and cancel without selecting | Target list unchanged; no error shown | |
| 2.4 | Add the same file twice via dialog | Second attempt shows duplicate toast; count stays at 1 | |

---

## 3. Monitoring with Auto-Refresh

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 3.1 | Add a file; open it in Notepad (keep open) | After next poll cycle, status changes to `Locked (1)` and `notepad.exe` appears in the process list | |
| 3.2 | Close Notepad | On next poll, status reverts to `Idle` and process list clears | |
| 3.3 | Change polling interval from 2 s to 5 s via the interval selector | Refresh rate slows to ~5 s; verify by timing status updates | |
| 3.4 | Change interval to 1 s | Refresh rate increases to ~1 s | |
| 3.5 | Add 10+ targets | All targets refresh on schedule without UI freeze | |

---

## 4. Force Terminate Non-System Process

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 4.1 | With a file locked by `notepad.exe`, click **Terminate** next to it | Confirmation dialog appears ("确认结束进程 notepad.exe?") | |
| 4.2 | Confirm termination | Notepad closes; target status refreshes to `Idle`; success toast shown | |
| 4.3 | Click **Terminate**, then cancel the confirmation dialog | Notepad remains open; no change to target list | |
| 4.4 | Lock a file with a process, terminate the process externally, then click **Terminate** in WhoLock | Toast shows "进程已不存在"; target list refreshes | |
| 4.5 | Attempt terminate while another terminate is in flight (rapid double-click) | Only one terminate executes; no crash or double-kill | |

---

## 5. System Process Protection (No Terminate Button)

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 5.1 | Add a file held by `svchost.exe` or another blacklisted process | `svchost.exe` appears in the process list **without** a Terminate button | |
| 5.2 | Inspect processes with PID 0 (Idle) or PID 4 (System) | No Terminate button shown for these entries | |
| 5.3 | Verify processes in `System32` running as SYSTEM SID show no Terminate button | Terminate button absent for system-classified processes | |
| 5.4 | Confirm `notepad.exe` (user process) still shows Terminate button | Terminate button present for non-system processes | |

---

## 6. Privilege Escalation Restart

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 6.1 | Run WhoLock as standard user; add a file held by a SYSTEM service | Target status shows `AccessDenied`; elevation banner/prompt appears | |
| 6.2 | Click **Restart as Administrator** | UAC prompt appears; accept it | |
| 6.3 | After UAC accept | WhoLock relaunches with elevated token; previous standard-user instance exits cleanly | |
| 6.4 | With elevated WhoLock, re-add the same file | Status now resolves correctly (no longer `AccessDenied`) | |
| 6.5 | Decline the UAC prompt | WhoLock remains running as standard user; no crash | |

---

## 7. Log File Writing and Opening

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 7.1 | Launch WhoLock | Log file `wholock-YYYY-MM-DD.log` created in `%LOCALAPPDATA%\FileLockInspector\logs\` | |
| 7.2 | Perform several actions (add target, terminate process, error) | Corresponding log entries written with timestamps | |
| 7.3 | Click **Help → Open Log Folder** | File Explorer opens to the log directory | |
| 7.4 | Run WhoLock on two consecutive days | Two separate daily log files created; no data loss between them | |
| 7.5 | Trigger an error (e.g., add missing path) | Error entry appears in the log with appropriate severity level | |

---

## 8. Unsupported OS Version Warning

| # | Step | Expected Result | Status |
|---|------|----------------|--------|
| 8.1 | Run WhoLock on Windows 10 < 1809 (build < 17763) or simulate via mock | Startup dialog shows "当前 Windows 版本不受支持"; main window does not open | |
| 8.2 | Dismiss the unsupported-OS dialog | Application exits cleanly (exit code 0) | |
| 8.3 | Run on Windows 10 1809+ or Windows 11 | No warning shown; application starts normally | |

---

## Sign-off

| Tester | Date | Build | Notes |
|--------|------|-------|-------|
| | | | |
