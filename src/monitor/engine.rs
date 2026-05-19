//! MonitorEngine：后台轮询调度线程（Requirements 3.1–3.8）

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{select, Receiver, Sender};

use crate::detector::restart_manager::RestartManagerApi;
use crate::error::ScanFailure;
use crate::monitor::clock::Clock;
use crate::monitor::scheduler::{ScanTaskRequest, SchedulerState};
use crate::monitor::{MonitorCmd, ScanEvent};
use crate::state::target::{TargetId, TargetItem, TargetKind, TargetStatus};

pub struct MonitorEngine {
    pub rm: Arc<dyn RestartManagerApi>,
    pub clock: Arc<dyn Clock>,
    pub cmd_rx: Receiver<MonitorCmd>,
    pub event_tx: Sender<ScanEvent>,
}

impl MonitorEngine {
    pub fn new(
        rm: Arc<dyn RestartManagerApi>,
        clock: Arc<dyn Clock>,
        cmd_rx: Receiver<MonitorCmd>,
        event_tx: Sender<ScanEvent>,
    ) -> Self {
        Self {
            rm,
            clock,
            cmd_rx,
            event_tx,
        }
    }

    pub fn run(self) {
        let mut scheduler = SchedulerState::new(Duration::from_millis(2000));

        let (scan_tx, scan_rx) = crossbeam_channel::bounded::<ScanJob>(16);
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<ScanResult>();

        // 4 个后台 worker 线程
        let _workers: Vec<_> = (0..4)
            .map(|_| {
                let scan_rx = scan_rx.clone();
                let result_tx = result_tx.clone();
                std::thread::spawn(move || {
                    while let Ok(job) = scan_rx.recv() {
                        let result = job.run();
                        let _ = result_tx.send(result);
                    }
                })
            })
            .collect();
        // 丢弃原始 scan_rx，只保留 worker 内部的克隆
        drop(scan_rx);

        let tick_dur = Duration::from_millis(100);

        loop {
            // 排空扫描结果
            while let Ok(r) = result_rx.try_recv() {
                scheduler.on_scan_done(r.id);
                let ev = match r.result {
                    Ok(records) => ScanEvent::Completed(r.id, records),
                    Err(failure) => ScanEvent::Failed(r.id, failure),
                };
                let _ = self.event_tx.send(ev);
            }

            select! {
                recv(self.cmd_rx) -> msg => match msg {
                    Ok(MonitorCmd::Shutdown) | Err(_) => break,
                    Ok(cmd) => {
                        let tasks = scheduler.apply(cmd);
                        self.dispatch(tasks, &scan_tx);
                    }
                },
                default(tick_dur) => {
                    let now = self.clock.now();
                    let tasks = scheduler.on_tick(now);
                    self.dispatch(tasks, &scan_tx);
                }
            }
        }

        // 丢弃发送端，让 worker 线程的 recv() 返回 Err 后退出
        drop(scan_tx);
    }

    fn dispatch(&self, tasks: Vec<ScanTaskRequest>, scan_tx: &Sender<ScanJob>) {
        for task in tasks {
            let _ = self.event_tx.send(ScanEvent::Started(task.id));
            let _ = scan_tx.send(ScanJob {
                id: task.id,
                path: task.path,
                rm: self.rm.clone(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// ScanJob / ScanResult：内部 worker 数据
// ---------------------------------------------------------------------------

struct ScanJob {
    id: TargetId,
    path: std::path::PathBuf,
    rm: Arc<dyn RestartManagerApi>,
}

struct ScanResult {
    id: TargetId,
    result: Result<Vec<crate::detector::ProcessRecord>, ScanFailure>,
}

impl ScanJob {
    fn run(self) -> ScanResult {
        let kind = if self.path.is_dir() {
            TargetKind::Directory
        } else {
            TargetKind::File
        };
        let target = TargetItem {
            id: self.id,
            path: self.path,
            kind,
            status: TargetStatus::Scanning,
            processes: vec![],
            last_scanned_at: None,
        };
        ScanResult {
            id: self.id,
            result: crate::detector::scan(&target, self.rm.as_ref()),
        }
    }
}
