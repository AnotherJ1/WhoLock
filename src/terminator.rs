//! 进程控制抽象 trait 及 Win32 实现。
//!
//! 提供 `ProcessControlApi` trait 以便在测试时注入 mock，同时
//! 包含生产实现 `Win32ProcessControl`。
//!
//! Requirements: 4.3, 4.4, 4.5, 4.6, 4.7, 5.5

use crossbeam_channel::RecvTimeoutError;
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

use crate::error::TerminateError;

// ---------------------------------------------------------------------------
// TerminateHandle：RAII 进程句柄包装
// ---------------------------------------------------------------------------

/// RAII 包装：持有通过 `OpenProcess` 取得的进程句柄，Drop 时自动关闭。
pub struct TerminateHandle(HANDLE);

impl Drop for TerminateHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessControlApi trait
// ---------------------------------------------------------------------------

/// 进程控制接口，用于 `open`、`get_start_time`、`terminate` 三步流程。
///
/// 设计为 `Send + Sync`，允许在后台线程中使用 `Arc<dyn ProcessControlApi>`。
pub trait ProcessControlApi: Send + Sync {
    /// 以终止权限打开目标进程，返回 RAII 句柄。
    ///
    /// # 错误
    /// - `TerminateError::AccessDenied`：`ERROR_ACCESS_DENIED`
    /// - `TerminateError::AlreadyExited`：`ERROR_INVALID_PARAMETER`（进程已不存在）
    /// - `TerminateError::Win32 { .. }`：其他 Win32 错误
    fn open_terminate(&self, pid: u32) -> Result<TerminateHandle, TerminateError>;

    /// 查询进程启动时间（`GetProcessTimes`），用于 PID 复用检测。
    fn get_start_time(&self, h: &TerminateHandle) -> Result<FILETIME, TerminateError>;

    /// 强制终止进程（`TerminateProcess`）。
    fn terminate(&self, h: &TerminateHandle, code: u32) -> Result<(), TerminateError>;
}

// ---------------------------------------------------------------------------
// Win32ProcessControl：生产实现
// ---------------------------------------------------------------------------

/// 直接调用 Win32 API 的生产实现。
pub struct Win32ProcessControl;

impl ProcessControlApi for Win32ProcessControl {
    fn open_terminate(&self, pid: u32) -> Result<TerminateHandle, TerminateError> {
        const ERROR_ACCESS_DENIED: u32 = 5;
        const ERROR_INVALID_PARAMETER: u32 = 87;

        let result = unsafe {
            OpenProcess(
                PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        };

        match result {
            Ok(handle) => Ok(TerminateHandle(handle)),
            Err(e) => {
                let code = e.code().0 as u32;
                match code {
                    ERROR_ACCESS_DENIED => Err(TerminateError::AccessDenied),
                    ERROR_INVALID_PARAMETER => Err(TerminateError::AlreadyExited),
                    _ => Err(TerminateError::from_win32(code)),
                }
            }
        }
    }

    fn get_start_time(&self, h: &TerminateHandle) -> Result<FILETIME, TerminateError> {
        let mut creation_time = FILETIME::default();
        let mut exit_time = FILETIME::default();
        let mut kernel_time = FILETIME::default();
        let mut user_time = FILETIME::default();

        let result = unsafe {
            GetProcessTimes(
                h.0,
                &mut creation_time,
                &mut exit_time,
                &mut kernel_time,
                &mut user_time,
            )
        };

        match result {
            Ok(()) => Ok(creation_time),
            Err(e) => {
                let code = e.code().0 as u32;
                Err(TerminateError::from_win32(code))
            }
        }
    }

    fn terminate(&self, h: &TerminateHandle, code: u32) -> Result<(), TerminateError> {
        let result = unsafe { TerminateProcess(h.0, code) };
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                let raw = e.code().0 as u32;
                Err(TerminateError::from_win32(raw))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// force_terminate：核心终止逻辑（需求 4.3, 4.5, 4.6, 4.7, 5.5）
// ---------------------------------------------------------------------------

/// 核心终止函数（同步，在专用线程中调用）。
///
/// 执行顺序：
/// 1. 系统进程短路保护（PID 0/4 → `SystemProtected`）
/// 2. `open_terminate` 打开句柄（失败透传 `TerminateError`）
/// 3. PID 复用防御：比较 `expected_start_time` 与实际启动时间（需求 4.6）
/// 4. 调用 `terminate`（需求 4.3）
pub fn force_terminate(
    pid: u32,
    expected_start_time: Option<FILETIME>,
    ctrl: &dyn ProcessControlApi,
) -> Result<(), TerminateError> {
    // 系统进程短路保护（需求 5.5）
    if pid == 0 || pid == 4 {
        return Err(TerminateError::SystemProtected);
    }

    let handle = ctrl.open_terminate(pid)?;

    // PID 复用防御（需求 4.6）
    if let Some(expected) = expected_start_time {
        let actual = ctrl.get_start_time(&handle)?;
        if actual.dwLowDateTime != expected.dwLowDateTime
            || actual.dwHighDateTime != expected.dwHighDateTime
        {
            return Err(TerminateError::StalePid);
        }
    }

    ctrl.terminate(&handle, 1)
}

// ---------------------------------------------------------------------------
// force_terminate_with_timeout：带 5s 超时包装（需求 4.4）
// ---------------------------------------------------------------------------

/// 带 5s 超时的包装：在专用线程中运行 `force_terminate`，主线程等待结果。
///
/// 若 5000ms 内未收到结果，返回 `Err(TerminateError::Timeout)`（需求 4.4）。
/// `FILETIME` 实现了 `Copy`，可安全跨线程传递。
pub fn force_terminate_with_timeout(
    pid: u32,
    expected_start_time: Option<FILETIME>,
) -> Result<(), TerminateError> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let ctrl = Win32ProcessControl;
        let result = force_terminate(pid, expected_start_time, &ctrl);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(Duration::from_millis(5000)) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(TerminateError::Timeout),
        Err(RecvTimeoutError::Disconnected) => Err(TerminateError::Timeout),
    }
}

// ---------------------------------------------------------------------------
// MockProcessControl：测试桩（cfg(test) only）
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 可脚本化返回值的测试桩。
    ///
    /// - `open_results`：`open_terminate` 调用依次消耗；耗尽后默认返回 `Ok`
    /// - `terminate_results`：`terminate` 调用依次消耗；耗尽后默认返回 `Ok`
    /// - `terminate_call_count`：记录 `terminate` 被调用的次数
    /// - `fake_start_time`：`get_start_time` 固定返回值
    pub struct MockProcessControl {
        pub open_results: Arc<Mutex<Vec<Result<(), TerminateError>>>>,
        pub terminate_results: Arc<Mutex<Vec<Result<(), TerminateError>>>>,
        pub terminate_call_count: Arc<Mutex<usize>>,
        pub fake_start_time: FILETIME,
    }

    impl MockProcessControl {
        pub fn new() -> Self {
            Self {
                open_results: Arc::new(Mutex::new(vec![])),
                terminate_results: Arc::new(Mutex::new(vec![])),
                terminate_call_count: Arc::new(Mutex::new(0)),
                fake_start_time: FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                },
            }
        }

        /// 追加一个 `open_terminate` 返回值到队列。
        pub fn with_open_result(self, r: Result<(), TerminateError>) -> Self {
            self.open_results.lock().unwrap().push(r);
            self
        }

        /// 追加一个 `terminate` 返回值到队列。
        pub fn with_terminate_result(self, r: Result<(), TerminateError>) -> Self {
            self.terminate_results.lock().unwrap().push(r);
            self
        }

        /// 设置 `get_start_time` 返回的伪造启动时间。
        pub fn with_fake_start_time(mut self, ft: FILETIME) -> Self {
            self.fake_start_time = ft;
            self
        }
    }

    impl ProcessControlApi for MockProcessControl {
        fn open_terminate(&self, _pid: u32) -> Result<TerminateHandle, TerminateError> {
            let mut q = self.open_results.lock().unwrap();
            if q.is_empty() {
                return Ok(TerminateHandle(HANDLE::default()));
            }
            q.remove(0).map(|_| TerminateHandle(HANDLE::default()))
        }

        fn get_start_time(&self, _h: &TerminateHandle) -> Result<FILETIME, TerminateError> {
            Ok(self.fake_start_time)
        }

        fn terminate(&self, _h: &TerminateHandle, _code: u32) -> Result<(), TerminateError> {
            *self.terminate_call_count.lock().unwrap() += 1;
            let mut q = self.terminate_results.lock().unwrap();
            if q.is_empty() {
                return Ok(());
            }
            q.remove(0)
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::mock::MockProcessControl;
    use super::*;
    use windows::Win32::Foundation::FILETIME;

    // --- system process guard ---

    #[test]
    fn force_terminate_blocks_pid_0() {
        let ctrl = MockProcessControl::new();
        assert!(matches!(
            force_terminate(0, None, &ctrl),
            Err(TerminateError::SystemProtected)
        ));
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 0);
    }

    #[test]
    fn force_terminate_blocks_pid_4() {
        let ctrl = MockProcessControl::new();
        assert!(matches!(
            force_terminate(4, None, &ctrl),
            Err(TerminateError::SystemProtected)
        ));
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 0);
    }

    // --- PID reuse / stale detection ---

    #[test]
    fn force_terminate_stale_pid_rejected_low_differs() {
        let ctrl = MockProcessControl::new().with_fake_start_time(FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        });
        let expected = FILETIME {
            dwLowDateTime: 1,
            dwHighDateTime: 0,
        };
        assert!(matches!(
            force_terminate(1234, Some(expected), &ctrl),
            Err(TerminateError::StalePid)
        ));
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 0);
    }

    #[test]
    fn force_terminate_stale_pid_rejected_high_differs() {
        let ctrl = MockProcessControl::new().with_fake_start_time(FILETIME {
            dwLowDateTime: 100,
            dwHighDateTime: 0,
        });
        let expected = FILETIME {
            dwLowDateTime: 100,
            dwHighDateTime: 1,
        };
        assert!(matches!(
            force_terminate(1234, Some(expected), &ctrl),
            Err(TerminateError::StalePid)
        ));
    }

    #[test]
    fn force_terminate_proceeds_when_start_time_matches() {
        let ft = FILETIME {
            dwLowDateTime: 42,
            dwHighDateTime: 7,
        };
        let ctrl = MockProcessControl::new().with_fake_start_time(ft);
        assert!(force_terminate(1234, Some(ft), &ctrl).is_ok());
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 1);
    }

    #[test]
    fn force_terminate_proceeds_with_no_expected_time() {
        let ctrl = MockProcessControl::new();
        assert!(force_terminate(1234, None, &ctrl).is_ok());
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 1);
    }

    // --- open_terminate error propagation ---

    #[test]
    fn force_terminate_propagates_access_denied_from_open() {
        let ctrl = MockProcessControl::new().with_open_result(Err(TerminateError::AccessDenied));
        assert!(matches!(
            force_terminate(1234, None, &ctrl),
            Err(TerminateError::AccessDenied)
        ));
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 0);
    }

    #[test]
    fn force_terminate_propagates_already_exited_from_open() {
        let ctrl = MockProcessControl::new().with_open_result(Err(TerminateError::AlreadyExited));
        assert!(matches!(
            force_terminate(999, None, &ctrl),
            Err(TerminateError::AlreadyExited)
        ));
    }

    // --- terminate error propagation ---

    #[test]
    fn force_terminate_propagates_terminate_error() {
        let ctrl =
            MockProcessControl::new().with_terminate_result(Err(TerminateError::AccessDenied));
        assert!(matches!(
            force_terminate(1234, None, &ctrl),
            Err(TerminateError::AccessDenied)
        ));
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 1);
    }

    // --- MockProcessControl: call sequence recording ---

    #[test]
    fn mock_records_multiple_terminate_calls() {
        let ctrl = MockProcessControl::new();
        let _ = force_terminate(100, None, &ctrl);
        let _ = force_terminate(200, None, &ctrl);
        let _ = force_terminate(300, None, &ctrl);
        assert_eq!(*ctrl.terminate_call_count.lock().unwrap(), 3);
    }

    #[test]
    fn mock_consumes_scripted_results_in_order() {
        let ctrl = MockProcessControl::new()
            .with_terminate_result(Err(TerminateError::AccessDenied))
            .with_terminate_result(Ok(()));
        let r1 = force_terminate(100, None, &ctrl);
        assert!(matches!(r1, Err(TerminateError::AccessDenied)));
        let r2 = force_terminate(200, None, &ctrl);
        assert!(r2.is_ok());
        // queue exhausted → defaults to Ok
        let r3 = force_terminate(300, None, &ctrl);
        assert!(r3.is_ok());
    }
}
