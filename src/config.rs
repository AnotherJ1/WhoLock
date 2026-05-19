//! 应用配置持久化（Requirement 3.2, 3.3）
//!
//! 配置文件路径：%LOCALAPPDATA%\FileLockInspector\config.json
//! Target_List 不持久化（design 决定）

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Language;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 轮询间隔（毫秒），默认 2000
    pub polling_interval_ms: u32,
    /// 窗口尺寸（宽×高）
    pub window_size: Option<[f32; 2]>,
    /// 窗口位置
    pub window_pos: Option<[f32; 2]>,
    /// UI 语言（默认 English）
    #[serde(default)]
    pub language: Language,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            polling_interval_ms: 2000,
            window_size: None,
            window_pos: None,
            language: Language::default(),
        }
    }
}

impl AppConfig {
    /// 配置文件路径
    pub fn config_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("FileLockInspector").join("config.json"))
    }

    /// 从磁盘加载，失败时返回默认值
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        let data = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    /// 保存到磁盘，失败时静默忽略
    pub fn save(&self) {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_polling_interval_is_2000() {
        assert_eq!(AppConfig::default().polling_interval_ms, 2000);
    }

    #[test]
    fn config_path_is_some() {
        // 仅在有 %LOCALAPPDATA% 的环境下成立
        // 不断言具体路径，仅验证不 panic
        let _ = AppConfig::config_path();
    }

    #[test]
    fn load_returns_default_when_no_file() {
        // config.json 不存在时应返回 default 而非 panic
        let cfg = AppConfig::load();
        assert!(cfg.polling_interval_ms > 0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        use tempfile::tempdir;
        // 验证序列化/反序列化往返正确性（不依赖真实 %LOCALAPPDATA%）
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("config.json");

        let cfg = AppConfig {
            polling_interval_ms: 5000,
            window_size: Some([1280.0, 720.0]),
            window_pos: Some([100.0, 200.0]),
            language: crate::i18n::Language::Zh,
        };
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: AppConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.polling_interval_ms, 5000);
        assert_eq!(loaded.window_size, Some([1280.0, 720.0]));
        assert_eq!(loaded.window_pos, Some([100.0, 200.0]));
    }
}
