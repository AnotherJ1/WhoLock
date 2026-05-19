use crate::state::target::{TargetId, TargetItem};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    Standard,
    Elevated,
}

#[derive(Debug, Clone)]
pub struct UiToast {
    pub message: String,
    pub is_error: bool,
}

impl UiToast {
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_error: false,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_error: true,
        }
    }
}

pub struct AppState {
    pub targets: BTreeMap<TargetId, TargetItem>,
    pub next_id: u64,
    pub polling_interval_ms: u32, // 1000 / 2000 / 5000 / 10000
    pub privilege: PrivilegeLevel,
    pub windows_supported: bool,
    pub last_error: Option<UiToast>,
}

impl AppState {
    pub fn new_default() -> Self {
        Self {
            targets: BTreeMap::new(),
            next_id: 1,
            polling_interval_ms: 2000,
            privilege: PrivilegeLevel::Standard,
            windows_supported: true,
            last_error: None,
        }
    }

    pub fn next_target_id(&mut self) -> TargetId {
        let id = TargetId(self.next_id);
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_default_has_expected_defaults() {
        let state = AppState::new_default();
        assert_eq!(state.next_id, 1);
        assert_eq!(state.polling_interval_ms, 2000);
        assert_eq!(state.privilege, PrivilegeLevel::Standard);
        assert!(state.windows_supported);
        assert!(state.last_error.is_none());
        assert!(state.targets.is_empty());
    }

    #[test]
    fn next_target_id_increments_monotonically() {
        let mut state = AppState::new_default();
        let id1 = state.next_target_id();
        let id2 = state.next_target_id();
        let id3 = state.next_target_id();
        assert_eq!(id1.0, 1);
        assert_eq!(id2.0, 2);
        assert_eq!(id3.0, 3);
    }

    #[test]
    fn ui_toast_info_not_error() {
        let t = UiToast::info("hello");
        assert_eq!(t.message, "hello");
        assert!(!t.is_error);
    }

    #[test]
    fn ui_toast_error_is_error() {
        let t = UiToast::error("bad thing");
        assert_eq!(t.message, "bad thing");
        assert!(t.is_error);
    }
}
