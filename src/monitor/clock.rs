//! `Clock` 抽象与真实实现。
//!
//! 调度器（`MonitorEngine`）需要"获取当前时刻"和"睡眠到下一个 tick"两类时间
//! 操作。生产环境直接用 `std::time::Instant::now` 与 `std::thread::sleep`；
//! 测试中需要 `FakeClock` 手动推进，因此把这两类操作抽到 trait 后面以便注入。
//!
//! 本文件只承载 trait 定义与 `SystemClock` 真实实现；`FakeClock` 在任务 7.2 中
//! 落地，调度器对 `Clock` 的具体使用在 Wave 5B（任务 9.x）中接入。

use std::time::{Duration, Instant};

/// 时间源抽象。
///
/// 必须 `Send + Sync`：`MonitorEngine` 会在后台 worker 线程中持有 `Arc<dyn Clock>`，
/// 既要从主线程派发命令，也要让多个 worker 共享同一时间源。
pub trait Clock: Send + Sync {
    /// 返回当前单调时刻。语义与 `std::time::Instant::now()` 一致。
    fn now(&self) -> Instant;

    /// 阻塞当前线程至少 `d` 时长。语义与 `std::thread::sleep(d)` 一致。
    fn sleep(&self, d: Duration);
}

/// 直接桥接到 `std` 标准库的真实实现。
///
/// 调度器在生产环境构造时使用 `Arc::new(SystemClock)`。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

// ---------------------------------------------------------------------------
// FakeClock：测试用手动推进时钟
// ---------------------------------------------------------------------------

use std::sync::Mutex;

/// 用于测试的手动推进时钟。
///
/// 内部以"基准 Instant + 累积偏移 Duration"的方式模拟时间推进，
/// 因为稳定 Rust 不支持直接构造任意 `Instant` 值。
/// 同时记录所有 `sleep` 调用的参数，供断言使用。
pub struct FakeClock {
    base: Instant,
    offset: Mutex<Duration>,
    sleeps: Mutex<Vec<Duration>>,
}

impl FakeClock {
    /// 以 `Instant::now()` 为基准创建新时钟，偏移为零。
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
            sleeps: Mutex::new(Vec::new()),
        }
    }

    /// 原子地将时钟向前推进 `d`。
    pub fn advance(&self, d: Duration) {
        *self.offset.lock().unwrap() += d;
    }

    /// 返回所有已记录的 `sleep` 调用参数（按调用顺序）。
    pub fn recorded_sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().unwrap().clone()
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    /// 返回 `base + 累积偏移`，模拟单调递增的时刻。
    fn now(&self) -> Instant {
        self.base + *self.offset.lock().unwrap()
    }

    /// 不实际睡眠，仅记录参数。
    fn sleep(&self, d: Duration) {
        self.sleeps.lock().unwrap().push(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小冒烟测试：`SystemClock::now` 是单调不减的。
    ///
    /// `Instant` 在所有受支持平台上都是单调时钟，第二次调用必然不早于第一次。
    /// 该测试主要用于校验 trait 实现可被构造与调用，不验证调度器逻辑
    /// （那部分留给 Property 4 / Property 6 在 `FakeClock` 上完成）。
    #[test]
    fn system_clock_now_is_monotonic() {
        let clock = SystemClock;
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(
            t2 >= t1,
            "Instant::now 必须单调不减：t1={:?}, t2={:?}",
            t1,
            t2
        );
    }
}
