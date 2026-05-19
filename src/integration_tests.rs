//! 集成测试模块（Wave 7 / Task 17.1-17.3）
//!
//! 这些测试**真实调用 Win32 API**：
//! - 启动 helper 子进程（`std::process::Command`）持有文件句柄
//! - 调用 `Win32RestartManager` 真实查询占用
//! - 调用 `force_terminate` 真实结束进程
//!
//! 全部用 `#[ignore]` 标记 — CI 默认跳过，开发者用
//! `cargo test -- --ignored` 手动触发。
//!
//! Requirements: 2.1, 2.2, 3.1, 3.2, 4.3, 4.4

#![cfg(test)]

use std::path::PathBuf;
use std::time::Duration;

use crate::detector::{scan, RestartManagerApi, Win32RestartManager};
use crate::state::target::{TargetId, TargetItem, TargetKind, TargetStatus};

/// 启动一个 cmd.exe 子进程，让它持有指定文件的写句柄。
/// 返回 (Child, file_path)，调用方负责后续 .kill() 或等待退出。
fn spawn_holder(file_path: &PathBuf) -> std::process::Child {
    // PowerShell 打开文件并 Start-Sleep 60s — 这会持有该文件句柄
    let script = format!(
        "$f = [System.IO.File]::Open('{}', 'OpenOrCreate', 'ReadWrite', 'None'); Start-Sleep -Seconds 60; $f.Close()",
        file_path.display()
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .spawn()
        .expect("failed to spawn powershell holder")
}

#[test]
#[ignore] // 需手动用 `cargo test -- --ignored` 运行
fn integration_lock_detection_finds_holder_pid() {
    // Requirements 2.1, 2.2: 加入 Target 后，Lock_Detector 能发现 Locking_Process
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("locked.txt");
    std::fs::write(&file_path, b"hello").unwrap();

    let mut child = spawn_holder(&file_path);
    let holder_pid = child.id();

    // 给 PowerShell 启动时间
    std::thread::sleep(Duration::from_millis(2000));

    let target = TargetItem {
        id: TargetId(1),
        path: file_path.clone(),
        kind: TargetKind::File,
        status: TargetStatus::Pending,
        processes: vec![],
        last_scanned_at: None,
    };
    let rm = Win32RestartManager;
    let result = scan(&target, &rm);

    let _ = child.kill();
    let _ = child.wait();

    let records = result.expect("scan should succeed");
    assert!(
        records.iter().any(|r| r.pid == holder_pid),
        "expected to find holder PID {} in records {:?}",
        holder_pid,
        records.iter().map(|r| r.pid).collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn integration_force_terminate_releases_lock() {
    // Requirements 4.3, 4.4: force_terminate 能真实结束目标进程
    use crate::terminator::{force_terminate, force_terminate_with_timeout, Win32ProcessControl};

    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("term.txt");
    std::fs::write(&file_path, b"x").unwrap();

    let mut child = spawn_holder(&file_path);
    let holder_pid = child.id();
    std::thread::sleep(Duration::from_millis(2000));

    let ctrl = Win32ProcessControl;
    let result = force_terminate(holder_pid, None, &ctrl);
    assert!(result.is_ok() || matches!(result, Err(crate::error::TerminateError::AlreadyExited)));

    // 等待进程实际退出
    std::thread::sleep(Duration::from_millis(1000));

    // 再次扫描应不再返回该 PID
    let target = TargetItem {
        id: TargetId(1),
        path: file_path.clone(),
        kind: TargetKind::File,
        status: TargetStatus::Pending,
        processes: vec![],
        last_scanned_at: None,
    };
    let rm = Win32RestartManager;
    let after = scan(&target, &rm).expect("scan after kill");
    assert!(
        !after.iter().any(|r| r.pid == holder_pid),
        "holder PID {} should be gone after terminate",
        holder_pid
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
#[ignore]
fn integration_monitor_engine_emits_periodic_started_events() {
    // Requirements 3.1, 3.2: MonitorEngine 周期性触发扫描
    use crate::monitor::clock::SystemClock;
    use crate::monitor::{MonitorCmd, MonitorEngine, ScanEvent};
    use crossbeam_channel::unbounded;
    use std::sync::Arc;

    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("watched.txt");
    std::fs::write(&file_path, b"y").unwrap();

    let (cmd_tx, cmd_rx) = unbounded::<MonitorCmd>();
    let (event_tx, event_rx) = unbounded::<ScanEvent>();

    let rm: Arc<dyn RestartManagerApi> = Arc::new(Win32RestartManager);
    let clock = Arc::new(SystemClock);
    let engine = MonitorEngine::new(rm, clock, cmd_rx, event_tx);
    let handle = std::thread::spawn(move || engine.run());

    // 添加 target
    cmd_tx
        .send(MonitorCmd::AddTarget {
            id: TargetId(1),
            path: file_path,
            kind: TargetKind::File,
        })
        .unwrap();

    // 观察 5 秒内应至少收到 2 次 Started（默认 2000ms 间隔）
    let mut started_count = 0;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        if let Ok(ev) = event_rx.recv_timeout(Duration::from_millis(500)) {
            if matches!(ev, ScanEvent::Started(_)) {
                started_count += 1;
            }
        }
    }

    cmd_tx.send(MonitorCmd::Shutdown).unwrap();
    let _ = handle.join();

    assert!(
        started_count >= 2,
        "expected ≥ 2 Started events in 6s with 2000ms interval, got {}",
        started_count
    );
}
