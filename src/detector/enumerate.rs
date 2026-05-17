//! 文件夹直接子项枚举工具。
//!
//! 对应需求 1.7：当 Target_Item 为文件夹时，Lock_Detector 同时检测
//! 该文件夹自身以及其直接子文件与一级子文件夹的占用情况，且不递归
//! 进入更深层级。
//!
//! 本模块只负责"枚举要注册到 Restart Manager 的路径集合"，不进行
//! 任何 Win32 调用。后续 Wave 5A（任务 8.1）的 `detector::scan` 会
//! 调用本函数取注册路径列表。

use std::path::{Path, PathBuf};

/// 返回 `[root]` ∪ root 的直接子文件 ∪ 直接子文件夹（深度恰好 = 1）。
///
/// # 行为
///
/// 1. 结果首项始终为 `root.to_path_buf()`，即便 `root` 不是目录或不存在。
/// 2. 若 `root` 不是目录或不存在，直接返回单元素 vec，不调用 `read_dir`。
/// 3. 若 `read_dir` 失败（如权限不足），记录 `tracing::warn!` 后返回单元素 vec。
/// 4. 遍历 `read_dir` 时，对每个失败的 entry 单独 `tracing::warn!` 并跳过。
/// 5. 不递归进入子目录（深度严格为 1）。
/// 6. 输出按路径字符串排序（root 始终位于首位），便于测试与展示稳定。
///
/// # 参数
///
/// * `root` - 要枚举的根路径
///
/// # 返回值
///
/// 至少包含 `root` 一项的 `Vec<PathBuf>`。
pub fn enumerate_direct_children(root: &Path) -> Vec<PathBuf> {
    let mut result = vec![root.to_path_buf()];

    // root 不是目录或不存在：直接返回，避免 read_dir 报无意义错误
    if !root.is_dir() {
        return result;
    }

    let entries = match std::fs::read_dir(root) {
        Ok(it) => it,
        Err(err) => {
            tracing::warn!(
                "read_dir failed for {}: {}",
                root.display(),
                err
            );
            return result;
        }
    };

    let mut children: Vec<PathBuf> = Vec::new();
    for entry in entries {
        match entry {
            Ok(e) => children.push(e.path()),
            Err(err) => {
                tracing::warn!(
                    "read_dir entry failed under {}: {}",
                    root.display(),
                    err
                );
            }
        }
    }

    // 为测试稳定性按路径字符串排序，root 已在首位（字符串短于其子项）
    children.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    result.extend(children);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn enumerates_root_and_only_direct_children() {
        let dir = tempdir().expect("create tempdir");
        let root = dir.path();

        // 直接子文件 x 2
        let file_a = root.join("a.txt");
        let file_b = root.join("b.log");
        fs::write(&file_a, b"hello").unwrap();
        fs::write(&file_b, b"world").unwrap();

        // 直接子目录 x 1，里面再放一个孙文件（深度 2）
        let sub = root.join("subdir");
        fs::create_dir(&sub).unwrap();
        let grand = sub.join("grandchild.txt");
        fs::write(&grand, b"deep").unwrap();

        let result = enumerate_direct_children(root);
        let set: HashSet<&Path> = result.iter().map(|p| p.as_path()).collect();

        // 必含 root + 2 文件 + 1 子目录，共 4 项
        assert_eq!(result.len(), 4, "expected 4 entries, got {:?}", result);
        assert!(set.contains(root), "result should contain root");
        assert!(set.contains(file_a.as_path()), "result should contain a.txt");
        assert!(set.contains(file_b.as_path()), "result should contain b.log");
        assert!(set.contains(sub.as_path()), "result should contain subdir");

        // 不能包含深度 ≥ 2 的孙文件
        assert!(
            !set.contains(grand.as_path()),
            "result must NOT contain grandchild (depth >= 2)"
        );
    }

    #[test]
    fn returns_single_element_when_root_is_file() {
        // root 指向一个文件而非目录：仍应至少包含 root 自身
        let dir = tempdir().expect("create tempdir");
        let file = dir.path().join("only.txt");
        fs::write(&file, b"x").unwrap();

        let result = enumerate_direct_children(&file);
        assert_eq!(result, vec![file]);
    }

    #[test]
    fn returns_single_element_when_root_missing() {
        // 不存在的路径：返回仅含 root 的 vec，不 panic
        let missing = Path::new("Z:\\definitely\\does\\not\\exist\\xyz123");
        let result = enumerate_direct_children(missing);
        assert_eq!(result, vec![missing.to_path_buf()]);
    }
}
