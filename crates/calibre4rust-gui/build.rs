//! 构建期把 `assets/icons` 下的 SVG 收集成静态表。
//!
//! 图标放在仓库根的 `assets/icons`(与 calibre 上游的 `imgsrc` 一一对应),
//! 位于 crate 包目录之外,因此不能直接用 `include_bytes!` 引用 —— 这里先把每个
//! SVG 复制到 `OUT_DIR/icons/` 下,再在生成的代码里用 `OUT_DIR` 相对路径引用。

use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icons_root = manifest_dir
        .join("..")
        .join("..")
        .join("assets")
        .join("icons");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let staged_root = out_dir.join("icons");

    let mut icons = Vec::new();
    collect_svgs(&icons_root, &icons_root, &mut icons);
    icons.sort();

    let mut generated = String::from("pub const ICONS: &[(&str, &[u8])] = &[\n");
    for relative in &icons {
        let staged = staged_root.join(relative);
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent).expect("create staged icon directory");
        }
        fs::copy(icons_root.join(relative), &staged).expect("stage icon file");

        let relative_string = relative.to_string_lossy().replace('\\', "/");
        let asset_path = format!("icons/{relative_string}");
        generated.push_str(&format!(
            "    ({asset_path:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/icons/{relative_string}\"))),\n"
        ));
    }
    generated.push_str("];\n");

    let generated_path = out_dir.join("icons_generated.rs");
    fs::write(&generated_path, generated).expect("write generated icon table");

    for relative in &icons {
        println!(
            "cargo:rerun-if-changed={}",
            icons_root.join(relative).to_string_lossy()
        );
    }
    // 目录本身也要盯着:在 `icons/plugins/` 里新增文件时,上面按文件列出的
    // `rerun-if-changed` 不会触发重建,需要额外盯住每个子目录。
    println!("cargo:rerun-if-changed={}", icons_root.to_string_lossy());
    for directory in sub_directories(&icons_root) {
        println!("cargo:rerun-if-changed={}", directory.to_string_lossy());
    }
}

fn sub_directories(root: &Path) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return directories;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            directories.push(path.clone());
            directories.extend(sub_directories(&path));
        }
    }
    directories
}

fn collect_svgs(root: &Path, directory: &Path, collected: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            panic!(
                "读取图标目录失败: {} ({error})",
                directory.to_string_lossy()
            );
        }
    };
    for entry in entries {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_svgs(root, &path, collected);
        } else if path.extension().is_some_and(|extension| extension == "svg") {
            let relative = path
                .strip_prefix(root)
                .expect("icon path is under the icons root")
                .to_path_buf();
            collected.push(relative);
        }
    }
}
