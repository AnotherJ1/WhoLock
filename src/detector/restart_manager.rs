//! Restart Manager API 抽象 trait 及相关数据类型。
//!
//! 通过 trait 而非直接调用 Win32，方便单元测试时注入 mock 实现，
//! 也便于未来替换底层实现（如模拟模式、集成测试桩）。

use std::path::PathBuf;

use windows::Win32::Foundation::FILETIME;

use crate::error::RmError;

/// Restart Manager 查询结果中的单个进程信息。
#[derive(Clone, Debug)]
pub struct RmProcessInfo {
    /// 进程 PID。
    pub pid: u32,
    /// 进程启动时间（由 Restart Manager 返回），用于 PID 复用检测。
    /// `None` 表示无法获取。
    pub start_time: Option<FILETIME>,
    /// 应用程序/进程名称（来自 RM_PROCESS_INFO.strAppName 或进程映像名）。
    pub app_name: String,
    /// 应用类型（对应 Win32 `RM_APP_TYPE`，以原始 u32 存储，避免直接依赖枚举映射）。
    pub app_type: u32,
}

/// Restart Manager 扫描接口。
///
/// 实现者负责：
/// 1. 建立 RM 会话（`RmStartSession`）。
/// 2. 注册资源路径（`RmRegisterResources`）。
/// 3. 查询锁定进程（`RmGetList`）。
/// 4. 关闭会话（`RmEndSession`）。
///
/// 每次 `scan` 调用均应完整地走完上述生命周期，以保证会话不泄漏。
pub trait RestartManagerApi: Send + Sync {
    /// 扫描给定路径集合，返回正在锁定这些路径的进程列表。
    ///
    /// # 错误
    ///
    /// - `RmError::AccessDenied`：系统拒绝访问（通常需要管理员权限）。
    /// - `RmError::PathNotFound`：路径不存在或不可访问。
    /// - `RmError::Win32 { .. }`：其他 Win32 错误。
    /// - `RmError::Other(_)`：逻辑错误或实现内部错误。
    fn scan(&self, paths: &[PathBuf]) -> Result<Vec<RmProcessInfo>, RmError>;
}
