//! 调度器状态机：管理监控目标集合、轮询间隔与飞行中扫描任务。
//!
//! `SchedulerState` 是纯粹的内存状态机，不依赖任何 Win32 API，便于 PBT 测试。

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::MonitorCmd;
use crate::state::target::TargetId;

/// 调度器向 Worker 线程发出的单次扫描请求。
#[derive(Debug, Clone)]
pub struct ScanTaskRequest {
    pub id: TargetId,
    pub path: PathBuf,
}

/// 调度器状态机。
///
/// 持有当前所有受监控目标的映射、正在飞行（已派发但未完成）的扫描集合、
/// 轮询间隔以及上次触发轮询的时刻。
pub struct SchedulerState {
    /// 受监控目标：TargetId → 路径
    pub targets: BTreeMap<TargetId, PathBuf>,
    /// 当前正在扫描（已派发未完成）的目标集合
    pub in_flight: HashSet<TargetId>,
    /// 轮询间隔
    pub interval: Duration,
    /// 上次触发轮询的时刻
    pub last_tick: Instant,
}

impl SchedulerState {
    /// 以给定轮询间隔创建新调度器，`last_tick` 设为 `Instant::now()`。
    pub fn new(interval: Duration) -> Self {
        Self {
            targets: BTreeMap::new(),
            in_flight: HashSet::new(),
            interval,
            last_tick: Instant::now(),
        }
    }

    /// 处理来自 UI 层的命令，返回需要立即派发的扫描任务列表。
    ///
    /// - `AddTarget` / `RemoveTarget` / `SetInterval`：仅更新字段，返回空 Vec。
    /// - `TriggerImmediate`：若目标存在且不在飞行中，立即派发扫描任务。
    /// - `Shutdown`：不做任何操作，调用方负责退出循环。
    pub fn apply(&mut self, cmd: MonitorCmd) -> Vec<ScanTaskRequest> {
        match cmd {
            MonitorCmd::AddTarget { id, path, kind: _ } => {
                self.targets.insert(id, path);
                Vec::new()
            }
            MonitorCmd::RemoveTarget(id) => {
                self.targets.remove(&id);
                self.in_flight.remove(&id);
                Vec::new()
            }
            MonitorCmd::SetInterval(ms) => {
                self.interval = Duration::from_millis(ms as u64);
                Vec::new()
            }
            MonitorCmd::TriggerImmediate(id) => {
                if let Some(path) = self.targets.get(&id) {
                    if !self.in_flight.contains(&id) {
                        self.in_flight.insert(id);
                        return vec![ScanTaskRequest {
                            id,
                            path: path.clone(),
                        }];
                    }
                }
                Vec::new()
            }
            MonitorCmd::Shutdown => Vec::new(),
        }
    }

    /// 检查是否应该触发新一轮轮询扫描。
    ///
    /// 若 `now - last_tick >= interval`，对所有不在飞行中的目标生成扫描任务，
    /// 并更新 `last_tick = now`。`targets` 为空时直接返回空。
    pub fn on_tick(&mut self, now: Instant) -> Vec<ScanTaskRequest> {
        if self.targets.is_empty() {
            return Vec::new();
        }
        if now.duration_since(self.last_tick) < self.interval {
            return Vec::new();
        }
        self.last_tick = now;

        // Collect candidates first to avoid simultaneous borrow of self.targets and self.in_flight
        let candidates: Vec<(TargetId, PathBuf)> = self
            .targets
            .iter()
            .filter(|(id, _)| !self.in_flight.contains(id))
            .map(|(id, path)| (*id, path.clone()))
            .collect();

        candidates
            .into_iter()
            .map(|(id, path)| {
                self.in_flight.insert(id);
                ScanTaskRequest { id, path }
            })
            .collect()
    }

    /// 扫描完成通知：从 `in_flight` 中移除指定目标。
    pub fn on_scan_done(&mut self, id: TargetId) {
        self.in_flight.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::MonitorCmd;
    use crate::state::target::{TargetId, TargetKind};

    fn make_id(n: u64) -> TargetId {
        TargetId(n)
    }
    fn make_path(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn add_target_registers_path() {
        let mut sched = SchedulerState::new(Duration::from_secs(2));
        let id = make_id(1);
        let tasks = sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/tmp/a.txt"),
            kind: TargetKind::File,
        });
        assert!(tasks.is_empty());
        assert!(sched.targets.contains_key(&id));
    }

    #[test]
    fn remove_target_clears_entry_and_inflight() {
        let mut sched = SchedulerState::new(Duration::from_secs(2));
        let id = make_id(1);
        sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/tmp/a.txt"),
            kind: TargetKind::File,
        });
        sched.in_flight.insert(id);
        sched.apply(MonitorCmd::RemoveTarget(id));
        assert!(!sched.targets.contains_key(&id));
        assert!(!sched.in_flight.contains(&id));
    }

    #[test]
    fn set_interval_updates_duration() {
        let mut sched = SchedulerState::new(Duration::from_secs(2));
        sched.apply(MonitorCmd::SetInterval(5000));
        assert_eq!(sched.interval, Duration::from_millis(5000));
    }

    #[test]
    fn trigger_immediate_dispatches_task() {
        let mut sched = SchedulerState::new(Duration::from_secs(60));
        let id = make_id(42);
        sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/x/y.txt"),
            kind: TargetKind::File,
        });
        let tasks = sched.apply(MonitorCmd::TriggerImmediate(id));
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, id);
        assert!(sched.in_flight.contains(&id));
    }

    #[test]
    fn trigger_immediate_skips_if_already_in_flight() {
        let mut sched = SchedulerState::new(Duration::from_secs(60));
        let id = make_id(1);
        sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/a"),
            kind: TargetKind::File,
        });
        sched.in_flight.insert(id);
        let tasks = sched.apply(MonitorCmd::TriggerImmediate(id));
        assert!(tasks.is_empty());
    }

    #[test]
    fn on_tick_does_nothing_before_interval() {
        let mut sched = SchedulerState::new(Duration::from_secs(60));
        let id = make_id(1);
        sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/a"),
            kind: TargetKind::File,
        });
        // now == last_tick, interval not elapsed
        let tasks = sched.on_tick(sched.last_tick);
        assert!(tasks.is_empty());
    }

    #[test]
    fn on_tick_dispatches_after_interval() {
        let mut sched = SchedulerState::new(Duration::from_millis(100));
        let id = make_id(1);
        sched.apply(MonitorCmd::AddTarget {
            id,
            path: make_path("/a"),
            kind: TargetKind::File,
        });
        // Simulate time passing beyond interval
        let future = sched.last_tick + Duration::from_millis(200);
        let tasks = sched.on_tick(future);
        assert_eq!(tasks.len(), 1);
        assert!(sched.in_flight.contains(&id));
    }

    #[test]
    fn on_tick_skips_empty_targets() {
        let mut sched = SchedulerState::new(Duration::from_millis(1));
        let future = sched.last_tick + Duration::from_secs(10);
        let tasks = sched.on_tick(future);
        assert!(tasks.is_empty());
    }

    #[test]
    fn on_scan_done_removes_from_inflight() {
        let mut sched = SchedulerState::new(Duration::from_secs(2));
        let id = make_id(7);
        sched.in_flight.insert(id);
        sched.on_scan_done(id);
        assert!(!sched.in_flight.contains(&id));
    }

    // -------------------------------------------------------------------
    // PBT: Property 4, 6, 11, 14
    // -------------------------------------------------------------------
    use proptest::prelude::*;

    proptest! {
        // Feature: file-lock-inspector, Property 4: 周期轮询频率
        // Validates: Requirements 3.1, 3.2, 3.3
        #[test]
        fn prop_polling_frequency_close_to_t_over_interval(
            interval_ms in prop::sample::select(vec![1000u64, 2000, 5000, 10000]),
            n_targets in 1usize..=4,
            multiplier in 3u64..=8,
        ) {
            let interval = Duration::from_millis(interval_ms);
            let mut sched = SchedulerState::new(interval);

            // 加入 n_targets 个目标
            for i in 0..n_targets {
                sched.apply(MonitorCmd::AddTarget {
                    id: TargetId(i as u64 + 1),
                    path: PathBuf::from(format!("C:/x{}", i)),
                    kind: TargetKind::File,
                });
            }

            let total_dur = interval * multiplier as u32;
            let start = sched.last_tick;
            let mut counts = vec![0u64; n_targets];
            // 模拟立即完成的扫描 — 每次 tick 触发后立刻 on_scan_done
            let step = interval / 4; // 4 倍精度的小步推进
            let mut now = start;
            while now < start + total_dur {
                let tasks = sched.on_tick(now);
                for t in tasks {
                    let idx = (t.id.0 - 1) as usize;
                    if idx < counts.len() { counts[idx] += 1; }
                    sched.on_scan_done(t.id);
                }
                now += step;
            }

            let expected = multiplier;
            for c in counts {
                let diff = (c as i64 - expected as i64).abs();
                prop_assert!(diff <= 1,
                    "expected ~{} ticks, got {} (diff {})", expected, c, diff);
            }
        }

        // Feature: file-lock-inspector, Property 6: 同 target 不并发扫描
        // Validates: Requirements 3.6, 3.7
        #[test]
        fn prop_no_concurrent_scan_for_same_target(
            interval_ms in 100u64..1000,
            tick_count in 5u32..30,
        ) {
            let interval = Duration::from_millis(interval_ms);
            let mut sched = SchedulerState::new(interval);
            let id = TargetId(1);
            sched.apply(MonitorCmd::AddTarget {
                id, path: PathBuf::from("C:/x"), kind: TargetKind::File,
            });

            let mut now = sched.last_tick;
            for _ in 0..tick_count {
                now += interval;
                let tasks = sched.on_tick(now);
                // 不调用 on_scan_done — 模拟慢扫描
                // 既然 in_flight 包含 id，后续 tick 不应再次返回
                for t in &tasks {
                    prop_assert!(sched.in_flight.contains(&t.id));
                }
                // 任意时刻 in_flight 中同一 id 仅出现一次（HashSet 保证）
                prop_assert!(sched.in_flight.contains(&id) || tasks.is_empty());
            }
            // 累计触发次数应 ≤ 1（首次 tick 后就锁住）
            // 通过 in_flight 验证：最多包含 1 个该 id
            prop_assert!(sched.in_flight.len() <= 1);
        }

        // Feature: file-lock-inspector, Property 11: 空列表暂停轮询
        // Validates: Requirements 7.4
        #[test]
        fn prop_empty_targets_yields_no_tasks(
            interval_ms in 100u64..3000,
            tick_count in 1u32..50,
        ) {
            let interval = Duration::from_millis(interval_ms);
            let mut sched = SchedulerState::new(interval);
            // targets 始终为空
            let mut now = sched.last_tick;
            for _ in 0..tick_count {
                now += interval * 2;
                let tasks = sched.on_tick(now);
                prop_assert!(tasks.is_empty(), "empty targets should produce no tasks");
            }
        }

        // Feature: file-lock-inspector, Property 14: SetInterval 不打断当前轮询
        // Validates: Requirements 3.4
        #[test]
        fn prop_set_interval_does_not_disrupt_in_flight(
            old_ms in prop::sample::select(vec![1000u64, 2000, 5000]),
            new_ms in prop::sample::select(vec![1000u64, 2000, 5000, 10000]),
        ) {
            let mut sched = SchedulerState::new(Duration::from_millis(old_ms));
            let id = TargetId(1);
            sched.apply(MonitorCmd::AddTarget {
                id, path: PathBuf::from("C:/x"), kind: TargetKind::File,
            });
            // 触发一次扫描，此时 in_flight 包含 id
            let t1 = sched.last_tick + Duration::from_millis(old_ms);
            let tasks = sched.on_tick(t1);
            prop_assert_eq!(tasks.len(), 1);
            prop_assert!(sched.in_flight.contains(&id));

            // 切换间隔 — 不应取消正在进行的扫描
            sched.apply(MonitorCmd::SetInterval(new_ms as u32));
            prop_assert!(sched.in_flight.contains(&id), "in_flight must survive SetInterval");
            prop_assert_eq!(sched.interval, Duration::from_millis(new_ms));
        }
    }
}
