pub mod actions;
pub mod app;
pub mod assets;
pub mod book_list;
pub mod cli;
pub mod service;
pub mod theme_provider;
pub mod window;

#[cfg(test)]
mod architecture_tests {
    use std::path::{Path, PathBuf};

    /// 只有 `service.rs` 可以依赖 caliberate 的 crate。
    ///
    /// 理由:caliberate 的 `ARCHITECTURE.md` 要求
    /// *"Protocol handlers must not independently reimplement database queries,
    /// search rules, or path-selection logic."* —— 前端不得绕过 service 层直接查 SQLite。
    #[test]
    fn only_service_module_imports_caliberate_crates() {
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
            for (index, line) in contents.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("use calibre")
                    || trimmed.starts_with("pub use calibre")
                    || trimmed.starts_with("use ::calibre")
                {
                    offenders.push(format!("{file_name}:{}", index + 1));
                }
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
