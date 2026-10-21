pub mod actions;
pub mod app;
pub mod assets;
pub mod book_list;
pub mod cli;
pub mod service;
pub mod theme_provider;
pub mod view_state;
pub mod window;

#[cfg(test)]
mod architecture_tests {
    use std::path::{Path, PathBuf};

    /// 只有 `service.rs` 可以依赖 caliberate 的 crate。
    ///
    /// 理由:caliberate 的 `ARCHITECTURE.md` 要求
    /// *"Protocol handlers must not independently reimplement database queries,
    /// search rules, or path-selection logic."* —— 前端不得绕过 service 层直接查 SQLite。
    ///
    /// 扫描**整个文件内容**而不是只扫 `use` 行:Rust 里不写 `use` 也能直接用
    /// 直接写 `Database::open_path(...)` 这样的全路径(连 `use` 都不用),只看 `use` 会被绕过。
    ///
    /// 目标串带下划线(caliberate 的 crate 名都是「caliberate」加下划线再跟名字),
    /// 这样散文里的 "caliberate" 不会误报;目标串在运行时拼接,免得这个测试自己的
    /// 源码命中自己。
    #[test]
    fn only_service_module_may_touch_backend_crates() {
        let needle = ["caliber", "ate_"].concat();
        let source_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for path in source_files(&source_directory) {
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if file_name == "service.rs" {
                continue;
            }
            let contents = std::fs::read_to_string(&path).expect("read source file");
            if contents.contains(&needle) {
                offenders.push(file_name);
            }
        }
        assert!(
            offenders.is_empty(),
            "只有 service.rs 可以依赖 caliberate crate,违反者: {offenders:?}"
        );
    }

    fn source_files(directory: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let entries = std::fs::read_dir(directory).expect("read source directory");
        for entry in entries {
            let entry = entry.expect("read directory entry");
            let path = entry.path();
            if path.is_dir() {
                files.extend(source_files(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
        files
    }
}
