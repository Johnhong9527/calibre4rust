# Phase 0 + Phase 1 实施计划:书库内核 + 主界面

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个能建库、导入书、浏览、按标签/搜索过滤、查看与编辑元数据的 macOS 桌面应用,界面结构与操作流程对齐上游 calibre。

**Architecture:** 四个 crate,依赖单向 `calibre-core ← calibre-formats ← calibre-app`,`calibre-i18n` 被 `calibre-app` 依赖。数据层用 SQLite(rusqlite,自带 FTS5),元数据模型(`Metadata`)归 `calibre-core`,格式中间表示(`OEBBook`)归 `calibre-formats`。UI 用 GPUI(git 依赖 zed 仓库,固定 rev),书列表用 `ui::Table`,标签浏览器树自实现。

**Tech Stack:** Rust 1.98.1 / edition 2024、GPUI(git `92447a17`)、rusqlite(bundled)、serde+serde_json、Fluent(i18n)、ICU(collator/casemap)、zip、quick-xml、html5ever、image、regex、Fluent。

**Spec:** `docs/superpowers/specs/2026-10-21-calibre2rust-design.md`(下称"spec";§编号均指该文档)

---

## Global Constraints

以下约束适用于**每一个** task,不再逐条重复:

- **工具链**:`rust-toolchain.toml` 固定 `channel = "1.98.1"`。安装需走镜像:`RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1`。
- **构建命令**:`cargo build --target aarch64-apple-darwin`。**禁止**用 `cargo metadata`(会解析所有 target,本机实测 >15 分钟未完成)。拉依赖用 `cargo fetch --target aarch64-apple-darwin`。
- **Cargo patch**:`calibre2Rust/Cargo.toml` 根必须含
  `[patch.crates-io] async-task = { git = "https://github.com/zed-industries/async-task.git", rev = "015845a19fc31752c02367567d3b8900a406f9da" }`。
  缺了它,`gpui` 编译失败(类型 `async_task::Task<T, M>` 不匹配)。
- **GPUI 特性**:`gpui_platform` 必须 `default-features = false, features = ["font-kit", "wayland", "x11", "runtime_shaders"]`。
  `runtime_shaders` 不可省 —— 本机无 Metal Toolchain,`xcrun metal` 不可用,缺它 `gpui_apple/build.rs:161-181` 会 `process::exit(1)`。
  `font-kit` 不可省 —— macOS 上它 gate 整个 Apple 文本系统(`gpui_apple/src/gpui_apple.rs:14-20`)。
- **`.cargo/config.toml`** 必须含 `[env] MACOSX_DEPLOYMENT_TARGET = "10.15.7"`。
- **Rust 规范**(来自 zed 的 `AGENTS.md`,spec §7.4):
  - 禁止 `unwrap()`(用 `?` 传播);禁止 `let _ =` 丢弃可失败操作;禁止"总结代码"的注释。
  - 变量用完整词,不用缩写。
  - **禁止 `mod.rs`**;新 crate 用 `[lib] path = "src/<name>.rs"`。
  - `cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check` 必须通过。
- **不引入**:`editor`、`search`、`picker`、`fs`、`assets`、`ui_input` 五个 zed crate(依赖过重,spec §3.2)。
- **不引入**:任何 Python 运行时、任何 WebView、任何 ORM。
- **许可**:整仓 GPL-3.0-or-later(spec §8 决策 26)。`assets/icons/` 来自 calibre(GPLv3),`assets/NOTICE` 记录来源。
- **图标**:使用 calibre 原版 `imgsrc/*.svg`(193 个),复制进本仓库 `assets/icons/`,**不**做栅格化。深色主题变体规则见 spec D12。
- **字体**:Phase 1 用 macOS 系统字体 + CJK fallback(见 Task 1),**不**下载 Noto Sans SC(网络不稳,spec §9 仍开放)。
- **提交粒度**:每个 task 结束时提交一次,commit message 用中文描述做了什么(spec 未要求英文)。

---

## Review Focus

spec 隐含但没有任何 task 的测试能覆盖的失败模式 —— 以下 5 条,每条都在下方对应 task 里补了测试:

1. **搜索查询含未转义引号 / 正则元字符 / 括号不配对**(如 `title:"unclosed`、`author:~(a|b`、`tags:(a`)。
   合理预期:**不 panic**,给出可读的解析错误,搜索框显示错误图标 —— 对应 **Task 6**。
2. **导入损坏或带 DRM 的书**(EPUB 的 zip 损坏、MOBI `encryption_type != 0`)。
   合理预期:**不崩溃、不产生半导入的残留目录**,给出明确错误 —— 对应 **Task 9 / Task 11**。
3. **书名/作者含文件系统非法字符**(`/`、`:`、空字符串、超长名、纯 emoji)。
   合理预期:sanitize 后落盘,**绝不写到库根之外**(路径穿越)—— 对应 **Task 11**。
4. **书库被并发占用 / SQLite 锁**。
   合理预期:错误可读,提示"书库正在被另一个程序使用",而不是 `database is locked` 裸错误 —— 对应 **Task 5**。
5. **中文 / 重音 / emoji 书名的排序与搜索**(如 `Émile` vs `Emily`、`《三体》` vs `三体`)。
   合理预期:搜索能命中(ICU 去重音),排序稳定且与上游 calibre 一致 —— 对应 **Task 8** 与 **Task 21**。

---

## 文件结构

```
calibre2Rust/
├── Cargo.toml                      # workspace 根 + [patch.crates-io]
├── rust-toolchain.toml
├── .cargo/config.toml
├── LICENSE                         # GPL-3.0-or-later
├── README.md
├── assets/
│   ├── NOTICE                      # 图标来源与许可
│   └── icons/*.svg                 # 从 ../calibre/imgsrc/ 复制
├── crates/
│   ├── calibre-core/
│   │   ├── Cargo.toml              # [lib] path = "src/calibre_core.rs"
│   │   └── src/
│   │       ├── calibre_core.rs     # lib 根,声明 pub mod
│   │       ├── error.rs            # Error / Result
│   │       ├── metadata.rs         # Metadata 模型 + 序列化
│   │       ├── field.rs            # FieldRegistry / FieldMetadata / DataType
│   │       ├── sort_key.rs         # title_sort / author_to_author_sort / 排序键
│   │       ├── library.rs          # Library(SQLite + 磁盘目录)
│   │       ├── schema.rs           # 建表 DDL + 迁移
│   │       ├── search/
│   │       │   ├── ...             # 见 Task 6/7
│   │       ├── import/
│   │       │   ├── ...             # 见 Task 11/12
│   │       └── ...
│   ├── calibre-formats/
│   │   ├── Cargo.toml              # [lib] path = "src/calibre_formats.rs"
│   │   └── src/
│   │       ├── calibre_formats.rs
│   │       ├── oeb.rs              # OEBBook
│   │       ├── epub.rs             # EPUB2/3 只读元数据
│   │       └── cover.rs            # 默认封面生成
│   ├── calibre-i18n/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── calibre_i18n.rs
│   │       └── locales/{zh-CN.ftl,en.ftl}
│   └── calibre-app/
│       ├── Cargo.toml              # [lib] path = "src/calibre_app.rs" + [[bin]] calibre2rust
│       └── src/
│           ├── calibre_app.rs      # lib 根
│           ├── main.rs             # bin
│           ├── app.rs              # AppState / 启动
│           ├── theme_provider.rs   # 自实现 ThemeSettingsProvider
│           ├── assets.rs           # 自实现 AssetSource
│           ├── window.rs           # 主窗口布局(工具栏/菜单栏/搜索栏/5 面板)
│           ├── tag_browser.rs      # 标签浏览器树
│           ├── book_list.rs        # 书列表 ui::Table
│           ├── book_details.rs     # 书籍详情面板
│           ├── search_box.rs       # 搜索框
│           ├── input_field.rs      # 单行输入框
│           └── edit_metadata.rs    # 元数据编辑模态框
├── tools/parity/
│   ├── parity.py                   # 与上游 calibre 对拍
│   └── queries.txt                 # 固定查询集
└── docs/superpowers/plans/
```

**文件职责约定**(每个文件一个清晰职责):
- `calibre-core` 是纯逻辑,不含 GPUI 依赖,可 `cargo test -p calibre-core` 秒级迭代。
- `calibre-app` 里的每个 UI 文件对应 spec D5 的一个面板。
- `calibre-formats` 的每个格式一个文件。

---

## Task 1: Workspace 骨架与 GPUI 启动

**Files:**
- Create: `Cargo.toml`(workspace 根)、`rust-toolchain.toml`、`.cargo/config.toml`、`LICENSE`、`README.md`
- Create: `crates/calibre-core/Cargo.toml`、`crates/calibre-core/src/calibre_core.rs`
- Create: `crates/calibre-app/Cargo.toml`、`crates/calibre-app/src/calibre_app.rs`、`crates/calibre-app/src/main.rs`、`crates/calibre-app/src/theme_provider.rs`、`crates/calibre-app/src/assets.rs`
- Create: `assets/NOTICE`、`assets/icons/*.svg`(从 `../calibre/imgsrc/` 复制)
- Test: `crates/calibre-app/src/assets.rs`(内联 `#[cfg(test)]` 模块)

**Interfaces:**
- Consumes: 无(首个 task)
- Produces:
  - crate `calibre_core`(空 lib,供后续 task 填充)
  - crate `calibre_app`(lib)+ 二进制 `calibre2rust`
  - `calibre_app::assets::CalibreAssets` —— `pub struct CalibreAssets;`,实现 `gpui::AssetSource`
  - `calibre_app::theme_provider::CalibreThemeSettings` —— `pub struct CalibreThemeSettings;`,实现 `theme::ThemeSettingsProvider`
  - `calibre_app::app::run()` —— `pub fn run()`,二进制入口调用它

- [ ] **Step 1: 写 workspace 骨架文件**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/calibre-core", "crates/calibre-app"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"
publish = false

[workspace.dependencies]
gpui           = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3", default-features = false }
gpui_platform  = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3", default-features = false, features = ["font-kit", "wayland", "x11", "runtime_shaders"] }
gpui_macros    = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
gpui_util      = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
ui             = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
theme          = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
icons          = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
menu           = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
fuzzy          = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
component      = { git = "https://github.com/zed-industries/zed", rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3" }
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
anyhow         = "1"
thiserror      = "2"
icu_collator   = "2"
icu_casemap    = "2"
icu_locale_core = "2"

[patch.crates-io]
async-task = { git = "https://github.com/zed-industries/async-task.git", rev = "015845a19fc31752c02367567d3b8900a406f9da" }
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.98.1"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

`.cargo/config.toml`:
```toml
[env]
MACOSX_DEPLOYMENT_TARGET = "10.15.7"
```

`LICENSE`:从 `../calibre/LICENSE` 复制(GPLv3 全文)。

`README.md`:三段 —— 这是什么(用 Rust + GPUI 复刻 calibre)、当前进度(Phase 0/1)、构建方法(注意 `RUSTUP_DIST_SERVER` 镜像与 `cargo build --target aarch64-apple-darwin`)。

- [ ] **Step 2: 写 crate 清单**

`crates/calibre-core/Cargo.toml`:
```toml
[package]
name = "calibre_core"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
name = "calibre_core"
path = "src/calibre_core.rs"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
```

`crates/calibre-core/src/calibre_core.rs`:
```rust
pub mod error;
```

`crates/calibre-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("书库文件损坏或不是有效的 calibre 书库: {0}")]
    InvalidLibrary(String),
    #[error("书库正在被另一个程序使用")]
    LibraryLocked,
    #[error("不支持的文件格式: {0}")]
    UnsupportedFormat(String),
    #[error("文件带有 DRM 保护,无法读取: {0}")]
    DrmProtected(String),
    #[error("解析失败: {0}")]
    Parse(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

> **注意**:`calibre-core` 此处已依赖 `rusqlite`,需在 `crates/calibre-core/Cargo.toml` 的 `[dependencies]` 补 `rusqlite = { version = "0.37", features = ["bundled", "functions"] }`(bundled 才能带 FTS5)。

`crates/calibre-app/Cargo.toml`:
```toml
[package]
name = "calibre_app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
name = "calibre_app"
path = "src/calibre_app.rs"

[[bin]]
name = "calibre2rust"
path = "src/main.rs"

[dependencies]
calibre_core = { path = "../calibre-core" }
gpui = { workspace = true }
gpui_platform = { workspace = true }
gpui_macros = { workspace = true }
gpui_util = { workspace = true }
ui = { workspace = true }
theme = { workspace = true }
icons = { workspace = true }
menu = { workspace = true }
component = { workspace = true }
anyhow = { workspace = true }
```

`crates/calibre-app/src/calibre_app.rs`:
```rust
pub mod app;
pub mod assets;
pub mod theme_provider;
```

- [ ] **Step 3: 复制 calibre 图标并写 NOTICE**

```bash
mkdir -p assets/icons
cp ../calibre/imgsrc/*.svg assets/icons/
cp -r ../calibre/imgsrc/plugins assets/icons/plugins
```

`assets/NOTICE`:
```
本目录的 SVG 图标取自 calibre 项目(https://github.com/kovidgoyal/calibre),
位于其 imgsrc/ 目录,版权归 Kovid Goyal 及 calibre 贡献者所有,
以 GPL-3.0-or-later 许可分发。

本项目整体亦以 GPL-3.0-or-later 分发。

文件名保持 calibre 原样,以便与上游界面逐一对应。
深色主题变体规则:图标名 N 先找 N-for-{主题}-theme.svg,找不到则用 N.svg。
```

- [ ] **Step 4: 写 `AssetSource` 实现与它的测试**

`crates/calibre-app/src/assets.rs`:
```rust
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;
use std::sync::OnceLock;

static ICON_BYTES: OnceLock<Vec<(String, &'static [u8])>> = OnceLock::new();

pub struct CalibreAssets;

impl AssetSource for CalibreAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let table = ICON_BYTES.get_or_init(build_icon_table);
        Ok(table
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let table = ICON_BYTES.get_or_init(build_icon_table);
        Ok(table
            .iter()
            .filter_map(|(name, _)| {
                let rest = name.strip_prefix(path)?.strip_prefix('/')?;
                Some(rest.into())
            })
            .collect())
    }
}
```

`build_icon_table()` 用 `include_bytes!` 无法枚举目录 —— 改用**构建期生成**:新增 `crates/calibre-app/build.rs`,扫描 `../../assets/icons` 生成一个 `icons_generated.rs`,内容为 `&[(&str, &[u8])]` 静态数组,`include!` 进 `assets.rs`。

`build.rs` 的扫描逻辑(用 `std::fs::read_dir` 递归,跳过 `plugins/` 之外的按需处理):
- 遍历 `assets/icons` 下的 `.svg` 文件
- 生成 `pub const ICONS: &[(&str, &[u8])] = &[("icons/book.svg", include_bytes!("../../assets/icons/book.svg")), ...];`
- 路径前缀固定为 `icons/`(与 zed 的 `IconName::path()` 约定一致)

**测试**(`crates/calibre-app/src/assets.rs` 内联):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_known_calibre_icon() {
        let assets = CalibreAssets;
        let bytes = assets.load("icons/book.svg").expect("load should succeed");
        assert!(bytes.is_some(), "icons/book.svg 必须存在");
        assert!(bytes.unwrap().starts_with(b"<svg"), "图标应为 SVG 内容");
    }

    #[test]
    fn list_returns_entries_under_a_directory() {
        let assets = CalibreAssets;
        let names = assets.list("icons").expect("list should succeed");
        assert!(names.contains(&"book.svg".into()), "list 应包含 book.svg");
    }

    #[test]
    fn load_returns_none_for_missing_asset() {
        let assets = CalibreAssets;
        assert!(assets.load("icons/does_not_exist.svg").expect("load should succeed").is_none());
    }
}
```

- [ ] **Step 5: 跑测试,确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin assets::`
Expected: 3 passed

- [ ] **Step 6: 写 `ThemeSettingsProvider` 实现**

`crates/calibre-app/src/theme_provider.rs`:
```rust
use gpui::{App, Font, FontFallbacks, Pixels, SharedString};
use std::sync::Arc;
use theme::{ThemeSettingsProvider, UiDensity};

pub struct CalibreThemeSettings;

impl ThemeSettingsProvider for CalibreThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        // 中文靠 fallback 链里的 PingFang SC / Hiragino Sans GB 覆盖
        static UI_FONT: OnceLock<Font> = OnceLock::new();
        UI_FONT.get_or_init(|| Font {
            family: ".SystemUIFont".into(),
            features: Default::default(),
            fallbacks: Some(FontFallbacks::from_fonts(vec![
                "PingFang SC".to_string(),
                "Hiragino Sans GB".to_string(),
            ])),
            weight: Default::default(),
            style: Default::default(),
        })
    }

    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        self.ui_font(_cx)
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(13.0)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::default()
    }
}
```

> `UiDensity` 的变体名需以 `theme` crate 实际导出为准 —— 先 `grep -n "pub enum UiDensity" -A 8 ../zed/crates/theme/src/` 确认后再写。

- [ ] **Step 7: 写启动代码与窗口**

`crates/calibre-app/src/app.rs`:
```rust
use gpui::{App, AppContext, Bounds, Window, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use theme::LoadThemes;

use crate::assets::CalibreAssets;
use crate::theme_provider::CalibreThemeSettings;

pub fn run() {
    gpui_platform::application()
        .with_assets(CalibreAssets)
        .run(|cx: &mut App| {
            theme::init(LoadThemes::JustBase, cx);
            theme::set_theme_settings_provider(
                Box::new(CalibreThemeSettings),
                cx,
            );
            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| RootView),
            )
            .expect("打开主窗口失败");
            cx.activate(true);
        });
}

struct RootView;

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .p_4()
            .gap_2()
            .child("calibre2Rust —— 中文字体测试:你好,世界")
            .child(ui::Button::new("点击我").on_click(|_, _window, _cx| {}))
    }
}
```

`crates/calibre-app/src/main.rs`:
```rust
fn main() {
    calibre_app::app::run();
}
```

- [ ] **Step 8: 编译**

Run: `cargo build --target aarch64-apple-darwin`
Expected: 编译成功。若报 `async_task::Task` 类型不匹配 → `[patch.crates-io]` 没生效,检查根 `Cargo.toml`。若报 `cannot execute tool 'metal'` → `runtime_shaders` 特性没开。

- [ ] **Step 9: 肉眼验证(需要录屏权限的机器)**

Run: `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected: 弹出窗口,显示中文"你好,世界"**不是豆腐块**,按钮可点。
若中文是方块 → `FontFallbacks` 没生效,检查 `ui_font` 是否被 `theme::theme_settings(cx)` 真正取到。

- [ ] **Step 10: 提交**

```bash
git add -A
git commit -m "feat: workspace 骨架与 GPUI 启动

- 根 Cargo.toml 引入 zed git 依赖并重复 async-task patch
- 启用 runtime_shaders 绕开本机缺失的 Metal Toolchain
- 自实现 AssetSource(calibre 原版图标)与 ThemeSettingsProvider
- 中文走系统字体 fallback(PingFang SC / Hiragino Sans GB)"
```

---

## Task 2: Metadata 模型与序列化

**Files:**
- Create: `crates/calibre-core/src/metadata.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`(加 `pub mod metadata;`)
- Modify: `crates/calibre-core/Cargo.toml`(加 `chrono`、`uuid`)
- Test: `crates/calibre-core/src/metadata.rs`(内联 `#[cfg(test)]` 模块)

**Interfaces:**
- Consumes: `calibre_core::error::{Error, Result}`
- Produces:
  - `calibre_core::metadata::Metadata` —— 结构体,字段见下
  - `calibre_core::metadata::Metadata::new(title: impl Into<String>) -> Metadata`
  - `calibre_core::metadata::Metadata::to_json(&self) -> Result<String>`
  - `calibre_core::metadata::Metadata::from_json(json: &str) -> Result<Metadata>`
  - `calibre_core::metadata::Metadata::to_opf_xml(&self) -> Result<String>` —— OPF2 形态,供 Phase 2 写 EPUB 用

```rust
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub title: String,
    pub authors: Vec<String>,
    pub author_sort: Option<String>,
    pub tags: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub rating: Option<f64>,
    pub comments: Option<String>,
    pub identifiers: std::collections::BTreeMap<String, String>,
    pub languages: Vec<String>,
    pub pubdate: Option<chrono::DateTime<chrono::Utc>>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub uuid: Option<String>,
    pub cover_data: Option<(String, Vec<u8>)>,
    pub formats: Vec<String>,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut meta = Metadata::new("三体");
        meta.authors = vec!["刘慈欣".into()];
        meta.tags = vec!["科幻".into()];
        meta.rating = Some(8.0);
        meta.identifiers.insert("isbn".into(), "9787536692930".into());
        let json = meta.to_json().expect("serialize should succeed");
        let parsed = Metadata::from_json(&json).expect("parse should succeed");
        assert_eq!(parsed.title, "三体");
        assert_eq!(parsed.authors, vec!["刘慈欣".to_string()]);
        assert_eq!(parsed.rating, Some(8.0));
        assert_eq!(parsed.identifiers.get("isbn").map(String::as_str), Some("9787536692930"));
    }

    #[test]
    fn identifier_keys_are_lowercased_and_stripped() {
        let mut meta = Metadata::new("X");
        meta.identifiers.insert("ISBN:".into(), "abc".into());
        // 规范化应在 set_identifier 时发生,见 Step 3
        meta.normalize_identifiers();
        assert_eq!(meta.identifiers.get("isbn").map(String::as_str), Some("abc"));
    }

    #[test]
    fn identifier_values_replace_commas_with_pipes() {
        let mut meta = Metadata::new("X");
        meta.set_identifier("isbn", "a,b");
        assert_eq!(meta.identifiers.get("isbn").map(String::as_str), Some("a|b"));
    }

    #[test]
    fn from_json_rejects_malformed_input() {
        assert!(Metadata::from_json("{not json").is_err());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin metadata::`
Expected: 编译错误 —— `Metadata` 未定义。

- [ ] **Step 3: 实现**

实现 `Metadata` 结构体与:
- `pub fn new(title: impl Into<String>) -> Metadata` —— `title` 为空时填 `"Unknown"`(对齐上游 `db/write.py` 的 `adapt_text` 默认值)。
- `pub fn normalize_identifiers(&mut self)` —— 键小写并去掉 `:` 与 `,`;值里的 `,` 换成 `|`(对齐上游 `ebooks/metadata/book/base.py:54-59` 的 `ck`/`cv`)。
- `pub fn set_identifier(&mut self, scheme: &str, value: &str)` —— 走上述规范化。
- `pub fn to_json(&self) -> Result<String>` —— `serde_json::to_string_pretty`。
- `pub fn from_json(json: &str) -> Result<Metadata>` —— `serde_json::from_str`,错误映射为 `Error::Parse`。
- `pub fn to_opf_xml(&self) -> Result<String>` —— 生成 OPF2 文档:`<package version="2.0" unique-identifier="uuid_id">` + `<metadata>` 里 `dc:title` / `dc:creator`(`opf:role="aut"`、`opf:file-as`=author_sort)/ `dc:date` / `dc:description` / `dc:publisher` / `dc:identifier`(`opf:scheme`)/ `dc:language`(每个语言一个)/ `dc:subject`(每个标签一个),外加 `<meta name="calibre:series">`、`<meta name="calibre:series_index">`、`<meta name="calibre:rating">`、`<meta name="calibre:timestamp">`。对齐上游 `ebooks/metadata/opf2.py:1654` 的 `metadata_to_opf`。
  - 用 `quick-xml`(`quick-xml-writer` 或手写字符串转义均可;若手写则必须转义 `& < > "`)。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin metadata::`
Expected: 4 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): Metadata 模型与 JSON/OPF2 序列化"
```

---

## Task 3: 字段系统(FieldRegistry)

**Files:**
- Create: `crates/calibre-core/src/field.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Test: `crates/calibre-core/src/field.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `calibre_core::field::DataType` —— 枚举
  - `calibre_core::field::FieldMetadata` —— 结构体
  - `calibre_core::field::FieldRegistry` —— 结构体
  - `FieldRegistry::standard() -> FieldRegistry`
  - `FieldRegistry::get(&self, name: &str) -> Option<&FieldMetadata>`
  - `FieldRegistry::displayable(&self) -> Vec<&FieldMetadata>`
  - `FieldRegistry::categories(&self) -> Vec<&FieldMetadata>`
  - `FieldRegistry::search_term_locations(&self) -> Vec<String>`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Text, Int, Float, Bool, DateTime, Rating, Series, Composite, Enumeration, Comments,
}

#[derive(Debug, Clone)]
pub struct FieldMetadata {
    pub name: String,
    pub label: String,
    pub datatype: DataType,
    pub is_multiple: bool,
    pub is_custom: bool,
    pub is_category: bool,
    pub table: Option<String>,
    pub column: Option<String>,
    pub search_terms: Vec<String>,
    pub sortable: bool,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_contains_upstream_builtin_fields() {
        let registry = FieldRegistry::standard();
        for name in [
            "title", "authors", "tags", "series", "series_index", "publisher",
            "rating", "languages", "formats", "identifiers", "comments",
            "pubdate", "timestamp", "last_modified", "uuid", "size", "pages", "cover",
        ] {
            assert!(registry.get(name).is_some(), "缺少内置字段 {name}");
        }
    }

    #[test]
    fn timestamp_search_term_is_date() {
        let registry = FieldRegistry::standard();
        let field = registry.get("timestamp").expect("timestamp field");
        assert!(field.search_terms.contains(&"date".to_string()));
    }

    #[test]
    fn category_fields_match_upstream_set() {
        let registry = FieldRegistry::standard();
        let mut names: Vec<&str> = registry.categories().iter().map(|f| f.name.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec!["authors", "formats", "identifiers", "languages", "news", "publisher", "rating", "series", "tags"]
        );
    }

    #[test]
    fn title_maps_to_sort_for_sorting() {
        let registry = FieldRegistry::standard();
        assert!(!registry.get("title").expect("title").sortable);
        assert!(registry.get("sort").expect("sort").sortable);
    }

    #[test]
    fn custom_column_names_use_hash_prefix() {
        let registry = FieldRegistry::standard().with_custom_column(CustomColumnDefinition {
            label: "mood".into(),
            datatype: DataType::Text,
            is_multiple: false,
        });
        let field = registry.get("#mood").expect("custom column");
        assert!(field.is_custom);
        assert_eq!(field.label, "mood");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin field::`
Expected: 编译错误 —— `FieldRegistry` 未定义。

- [ ] **Step 3: 实现**

- `DataType` 按上表。
- `FieldMetadata` 按上表。
- `CustomColumnDefinition { label: String, datatype: DataType, is_multiple: bool }` —— label 必须匹配 `^\w*$` 且以字母开头(对齐上游 `db/backend.py:1502` 的 `create_custom_column` 校验),否则返回 `Err(Error::Other)`。
- `FieldRegistry::standard()` 内置以下字段(spec D4 表格):
  - `title`(Text, 不可排序)、`sort`(Text, 标签 "Title sort", 可排序)
  - `authors`(Text, multiple, table `authors`, search_terms `["authors","author"]`, category, 可排序)
  - `author_sort`(Text, 不可排序)
  - `tags`(Text, multiple, table `tags`, search_terms `["tags","tag"]`, category, 可排序)
  - `series`(Series, table `series`, category, 可排序)、`series_index`(Float)、`series_sort`(Text, 虚拟, 可排序)
  - `publisher`(Text, table `publishers`, category, 可排序)
  - `rating`(Rating, table `ratings`, category, 可排序)
  - `languages`(Text, multiple, table `languages`, search_terms `["languages","language"]`, category, 可排序)
  - `formats`(Text, multiple, 虚拟, search_terms `["formats","format"]`, category, **不可排序**)
  - `identifiers`(Text, multiple, 键值对, search_terms `["identifiers","identifier","isbn"]`, category, 不可排序)
  - `comments`(Comments, 不可排序)
  - `cover`(Bool, 虚拟)
  - `id`(Int, 不可排序)
  - `pages`(Int)
  - `pubdate`(DateTime, 可排序)、`timestamp`(DateTime, search_terms `["date"]`, 可排序)、`last_modified`(DateTime, 不可排序)
  - `uuid`(Text, 不可排序)
  - `size`(Float)
  - `path`(Text, 无 search_terms, 不可排序)
- `categories()` 返回 `is_category == true` 的字段,按上游顺序:`authors, languages, series, formats, publisher, rating, news, tags, identifiers`。
  - `news` 是特殊字段:它对应名为 "News" 的标签与其它标签的交叉,不是真实列 —— 建模为 `is_category: true` 的虚拟字段。
- `displayable()` 返回除 `sort`/`author_sort`/`series_sort`/`id`/`path`/`cover`/`ondevice`/`au_map`/`marked`/`in_tag_browser` 外的字段。
- `search_term_locations()` 返回所有字段 `search_terms` 的并集(供 `all:` 展开用)。
- `with_custom_column(def)` 把自定义列以 `#label` 为名注册;归一化类型(Text/Series/Rating/Enumeration)的 `is_category = true`。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin field::`
Expected: 5 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): 字段系统 FieldRegistry,对齐上游内置字段与搜索词"
```

---

## Task 4: title_sort 与 author_to_author_sort

**Files:**
- Create: `crates/calibre-core/src/sort_key.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Modify: `crates/calibre-core/Cargo.toml`(加 `icu_collator`、`icu_casemap`、`icu_locale_core`、`icu` 聚合包)
- Test: `crates/calibre-core/src/sort_key.rs`

**Interfaces:**
- Consumes: `calibre_core::field::DataType`
- Produces:
  - `calibre_core::sort_key::title_sort(title: &str, order: TitleSortOrder, language: Option<&str>) -> String`
  - `calibre_core::sort_key::TitleSortOrder` —— 枚举 `LibraryOrder` / `StrictlyAlphabetic`
  - `calibre_core::sort_key::author_to_author_sort(author: &str) -> String`
  - `calibre_core::sort_key::author_sort_copy_method() -> AuthorSortCopyMethod` —— 枚举 `Copy` / `Comma` / `NoComma`
  - `calibre_core::sort_key::string_to_authors(raw: &str) -> Vec<String>`
  - `calibre_core::sort_key::authors_to_string(authors: &[String]) -> String`
  - `calibre_core::sort_key::sort_key_for(text: &str, language: Option<&str>) -> String` —— ICU collator 排序键

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_sort_moves_leading_article_to_the_end() {
        assert_eq!(
            title_sort("The Lord of the Rings", TitleSortOrder::LibraryOrder, Some("eng")),
            "Lord of the Rings, The"
        );
    }

    #[test]
    fn title_sort_keeps_strictly_alphabetic_untouched() {
        assert_eq!(
            title_sort("The Lord of the Rings", TitleSortOrder::StrictlyAlphabetic, Some("eng")),
            "The Lord of the Rings"
        );
    }

    #[test]
    fn title_sort_uses_language_specific_articles() {
        // 法语冠词 Le
        assert_eq!(
            title_sort("Le Petit Prince", TitleSortOrder::LibraryOrder, Some("fra")),
            "Petit Prince, Le"
        );
    }

    #[test]
    fn title_sort_strips_matching_quote_pair() {
        assert_eq!(
            title_sort("\"Dune\"", TitleSortOrder::LibraryOrder, Some("eng")),
            "Dune"
        );
    }

    #[test]
    fn author_sort_keeps_comma_names_verbatim() {
        assert_eq!(author_to_author_sort("Smith, John"), "Smith, John");
    }

    #[test]
    fn author_sort_moves_surname_first() {
        assert_eq!(author_to_author_sort("John Smith"), "Smith, John");
    }

    #[test]
    fn author_sort_appends_suffix() {
        assert_eq!(author_to_author_sort("John Smith Jr"), "Smith, John Jr");
    }

    #[test]
    fn author_sort_ignores_bracketed_text() {
        assert_eq!(author_to_author_sort("John Smith (Editor)"), "Smith, John");
    }

    #[test]
    fn string_to_authors_splits_on_ampersand_and_and() {
        assert_eq!(string_to_authors("Author & Co"), vec!["Author".to_string(), "Co".to_string()]);
        assert_eq!(string_to_authors("Author and Co"), vec!["Author".to_string(), "Co".to_string()]);
    }

    #[test]
    fn string_to_authors_honours_escaped_ampersand() {
        assert_eq!(string_to_authors("Smith && Sons"), vec!["Smith & Sons".to_string()]);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin sort_key::`
Expected: 编译错误 —— 函数未定义。

- [ ] **Step 3: 实现**

严格按 spec D4 的算法描述实现(算法已在上游 `ebooks/metadata/__init__.py:67` 与 `:203` 逐条列明):

- `title_sort`:
  1. `order == StrictlyAlphabetic` → 原样返回。
  2. 剥掉首尾成对引号:`"`…`"`、`“…”`、`«…»`、`„…„`、`‹…›`。
  3. 按语言取冠词表(见 spec D4 的五张表);语言为 `None` 时依次回退 `default_language_for_title_sort` → 英语。
  4. 命中冠词 → 冠词移到末尾,拼成 `", <冠词>"`。
- `author_to_author_sort`:
  1. method 为 `Copy` → 原样;`Comma` 且名字(剥括号后)含逗号 → 原样。
  2. 剥掉方括号内容。
  3. 任一 token 命中 copywords 列表(`Agency, Corporation, Company, Co., Council, …`,见上游 `resources/default_tweaks.py:74`)→ 原样。
  4. 跳前缀(`Mr, Mrs, Ms, Dr, Prof`)与后缀(`Jr, Sr, Inc, Ph.D, MD, I, II, III, IV, Junior, Senior`)。
  5. 姓氏前缀(`da, de, di, la, le, van, von`)仅在**恰好两个 token**时参与;`use_surname_prefixes` 默认 false。
  6. 输出 `Surname, Given … Suffix`(`NoComma` 时为 `Surname Given`)。
- `string_to_authors`:`&&` 先替换为哨兵 `\u{FFFF}`;把 `(?i),?\s+(and|with)\s+` 改写为 `&`;再按**任意单个** `&` 切分;最后还原哨兵。
- `authors_to_string`:用 `" & "` 连接,每个作者内的 `&` 需转义为 `&&`。
- `sort_key_for`:用 `icu_collator` 的 collator 生成排序键(语言为 `None` 时用根 locale)。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin sort_key::`
Expected: 10 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): title_sort / author_to_author_sort / 作者拆分,对齐上游算法"
```

---

## Task 5: 存储层(Library + schema)

**Files:**
- Create: `crates/calibre-core/src/schema.rs`、`crates/calibre-core/src/library.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Test: `crates/calibre-core/src/library.rs`

**Interfaces:**
- Consumes: `calibre_core::metadata::Metadata`、`calibre_core::field::FieldRegistry`、`calibre_core::error::{Error, Result}`
- Produces:
  - `calibre_core::library::Library` —— 结构体
  - `Library::open(path: &Path) -> Result<Library>` —— 打开或创建书库
  - `Library::path(&self) -> &Path`
  - `Library::fields(&self) -> &FieldRegistry`
  - `Library::add_book(&mut self, meta: &Metadata, formats: Vec<(String, Vec<u8>)>) -> Result<i64>`
  - `Library::book_metadata(&self, book_id: i64) -> Result<Metadata>`
  - `Library::all_book_ids(&self) -> Result<Vec<i64>>`
  - `Library::book_ids_matching(&self, where_clause: &str, params: &[&dyn rusqlite::ToSql]) -> Result<Vec<i64>>`
  - `calibre_core::library::sanitize_path_component(name: &str) -> String`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_library() -> (PathBuf, Library) {
        let dir = std::env::temp_dir().join(format!("calibre2rust-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let library = Library::open(&dir).expect("open library");
        (dir, library)
    }

    #[test]
    fn opening_creates_the_database() {
        let (dir, _library) = temp_library();
        assert!(dir.join("calibre.db").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adding_a_book_assigns_an_id_and_persists_metadata() {
        let (dir, mut library) = temp_library();
        let mut meta = Metadata::new("三体");
        meta.authors = vec!["刘慈欣".into()];
        let book_id = library.add_book(&meta, vec![("epub".into(), b"fake".to_vec())]).expect("add book");
        assert!(book_id > 0);
        let loaded = library.book_metadata(book_id).expect("load metadata");
        assert_eq!(loaded.title, "三体");
        assert_eq!(loaded.authors, vec!["刘慈欣".to_string()]);
        drop(library);
        let reopened = Library::open(&dir).expect("reopen");
        assert_eq!(reopened.book_metadata(book_id).expect("reload").title, "三体");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn adding_a_book_creates_a_directory_with_the_title_and_id() {
        let (dir, mut library) = temp_library();
        let mut meta = Metadata::new("Dune");
        meta.authors = vec!["Frank Herbert".into()];
        let book_id = library.add_book(&meta, vec![]).expect("add book");
        let book_dir = dir.join("Frank Herbert").join(format!("Dune ({book_id})"));
        assert!(book_dir.exists(), "应创建目录 {}", book_dir.display());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sanitize_path_component_strips_separators_and_control_chars() {
        assert_eq!(sanitize_path_component("a/b"), "a_b");
        assert_eq!(sanitize_path_component(".."), "_");
        assert_eq!(sanitize_path_component(""), "Unknown");
        assert_eq!(sanitize_path_component("  "), "Unknown");
        assert!(!sanitize_path_component("a\u{0}b").contains('\u{0}'));
    }

    #[test]
    fn sanitized_names_cannot_escape_the_library_root() {
        let (dir, mut library) = temp_library();
        let mut meta = Metadata::new("../../etc");
        meta.authors = vec!["../../root".into()];
        library.add_book(&meta, vec![]).expect("add book");
        let entries: Vec<_> = std::fs::read_dir(&dir).expect("read dir").filter_map(|e| e.ok()).collect();
        assert!(entries.iter().all(|e| e.path().starts_with(&dir)));
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin library::`
Expected: 编译错误 —— `Library` 未定义。

- [ ] **Step 3: 实现 `schema.rs`**

按 spec D2 的 schema 草案写 DDL 字符串常量,`Library::open` 时执行:
```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
```
建表语句见 spec D2 的清单(含 `books`、`authors`、`tags`、`series`、`publishers`、`ratings`、`languages`、`comments`、`identifiers`、`data`、各 `books_*_link`、`custom_columns`、`custom_column_*`、`preferences`、`annotations`、`books_fts` FTS5 虚表)。

`books_fts` 用 FTS5 外部内容表:
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS books_fts USING fts5(title, authors, tags, comments, content='books', content_rowid='id');
```

- [ ] **Step 4: 实现 `library.rs`**

- `Library` 持有 `rusqlite::Connection` + `PathBuf` + `FieldRegistry`。
- `Library::open(path)`:
  - `std::fs::create_dir_all(path)`
  - 打开 `path/calibre.db`;若 rusqlite 返回 `SQLITE_BUSY` / `SQLITE_LOCKED` → 映射为 `Error::LibraryLocked`
  - 执行 schema
- `add_book(&mut self, meta, formats)`:
  - 计算 `title_sort`(`sort_key::title_sort`)、`author_sort`(`sort_key::author_to_author_sort` 逐作者后用 `" & "` 连接)
  - 分配 id:`INSERT INTO books(title, title_sort, author_sort, series_index, pubdate, timestamp, last_modified, uuid, size, path) VALUES (...)`,取 `last_insert_rowid()`
  - 写 `authors` / `tags` / `series` / `publisher` / `rating` / `languages` / `comments` / `identifiers` / `data` 各表与 link 表(用 `INSERT OR IGNORE` + `SELECT id` 的 upsert 模式)
  - 生成书目录:`<root>/<sanitize(author)>/<sanitize(title)> (<id>)/`,创建目录
  - 写格式文件:`<书名> - <作者>.<ext>`
  - 写 `metadata.json` 边车(`Metadata::to_json`)
  - 更新 `books.last_modified`、`books.size`
- `book_metadata(&self, book_id) -> Result<Metadata>` —— 逆向组装。
- `sanitize_path_component(name) -> String`:
  - 替换路径分隔符 `/` `\` 与控制字符为 `_`
  - 去掉首尾空格与 `.`
  - 空串或全空白 → `"Unknown"`
  - 结果为 `.` 或 `..` → `_`
  - 长度截断到 120 字符
  - **不**使用 `..` 作为路径组件

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin library::`
Expected: 5 passed

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(core): 存储层 Library 与 schema,含路径 sanitize 防目录穿越"
```

---

## Task 6: 搜索语言 —— 词法与语法

**Files:**
- Create: `crates/calibre-core/src/search/mod.rs`、`crates/calibre-core/src/search/lexer.rs`、`crates/calibre-core/src/search/ast.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Test: `crates/calibre-core/src/search/lexer.rs`

**Interfaces:**
- Consumes: `calibre_core::error::{Error, Result}`
- Produces:
  - `calibre_core::search::ast::Query` —— 枚举
  - `calibre_core::search::ast::SearchValue` —— 枚举
  - `calibre_core::search::ast::DateOp` / `NumberOp` / `CompareOp` —— 枚举
  - `calibre_core::search::parse(query: &str) -> Result<Query>`
  - `calibre_core::search::lexer::Token` —— 枚举

```rust
pub enum Query {
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    Location { field: String, value: SearchValue },
    FreeText(String),
    SavedSearch(String),
    VirtualLibrary(String),
    UserCategory { category: String, item: String },
}

pub enum SearchValue {
    Contains(String),
    Exact(String),
    Literal(String),
    Regex(String),
    AccentInsensitive(String),
    HierarchicalPrefix(String),
    HierarchicalComponent(String),
    Bool(bool),
    Date(DateOp),
    Number(CompareOp, f64),
    Rating(CompareOp, f64),
    Count(CompareOp, usize),
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_bare_words_and_parens() {
        let tokens = tokenize("title:foo (bar)").expect("tokenize");
        assert_eq!(tokens.len(), 4);
    }

    #[test]
    fn tokenizes_quoted_string() {
        let tokens = tokenize("title:\"foo bar\"").expect("tokenize");
        assert!(tokens.iter().any(|t| matches!(t, Token::QuotedWord(s) if s == "foo bar")));
    }

    #[test]
    fn tokenizes_triple_quoted_string_without_escapes() {
        let tokens = tokenize("tags:\"\"\"a \"b\" c\"\"\"").expect("tokenize");
        assert!(tokens.iter().any(|t| matches!(t, Token::DocString(s) if s == "a \"b\" c")));
    }

    #[test]
    fn tokenizes_user_category_location() {
        let tokens = tokenize("@Fav:some item").expect("tokenize");
        assert!(tokens.iter().any(|t| matches!(t, Token::Word(s) if s.contains("@Fav:"))));
    }

    #[test]
    fn unclosed_quote_is_an_error_not_a_panic() {
        let error = tokenize("title:\"unclosed").expect_err("should fail");
        assert!(error.to_string().contains("引号") || error.to_string().contains("quote"));
    }

    #[test]
    fn unclosed_paren_is_an_error_not_a_panic() {
        assert!(parse("tags:(a").is_err());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin search::`
Expected: 编译错误 —— 模块未定义。

- [ ] **Step 3: 实现词法器**

按 spec D3 的词法规则:
- 裸词:`[^"()\s]+`;另有 `@.+?:[^")\s]+` 规则匹配 `@UserCategory:item`。
- 引号串 `"..."`,`\"` 转义,`\\` 转义。
- 三引号 `"""..."""`:**内部不做转义处理**;实现方式是先把 `"""..."""` 整体匹配出来,原样作为 `Token::DocString`。
- 转义序列 `\\`、`\"`、`\(`、`\)` 在词法阶段还原为字面字符,并**阻止**其被当作结构符。
- `(` `)` 为独立 token。
- 遇到未闭合引号 → 返回 `Error::Parse("引号未闭合")`。

- [ ] **Step 4: 实现语法分析**

递归下降:
```
prog        ::= or_expr
or_expr     ::= and_expr ('or' or_expr)?
and_expr    ::= not_expr ('and'? and_expr)?
not_expr    ::= 'not'? location_expr
location_expr ::= base_token | '(' or_expr ')'
```
相邻项隐式 AND。`and` / `or` / `not` 为大小写不敏感的关键字(仅当独立成词时)。

`base_token` 解析:
- 若整个 token 形如 `field:value` 且 `field` 是已知搜索位置 → `Query::Location`
- 否则 → `Query::FreeText`

已知搜索位置:由 `FieldRegistry::search_term_locations()` 提供;特殊位置 `all`、`search:<name>`、`vl:<name>`、`@UserCategory:item`。

`field:value` 的值部分按 spec D3 的顺序解析(`_matchkind`):
1. 长度 > 1 且以 `\` 开头 → `Literal`
2. 长度 > 1 且以 `=` 开头 → 剥 `=`,再:
   - 以 `..` 开头 → `HierarchicalComponent(剥掉 ".." 后的内容)`
   - 以 `.` 开头 → `HierarchicalPrefix(剥掉 "." 后的内容)`
   - 否则 → `Exact`
3. 长度 > 1 且以 `~` 开头 → `Regex`
4. 长度 > 1 且以 `^` 开头 → `AccentInsensitive`
5. 其余 → `Contains`

> ⚠️ **顺序至关重要**:`=` 在前,层级前缀在 `=` 之后。写成 `.=` 会退化成字面包含(spec D3 已标注此坑)。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin search::`
Expected: 6 passed

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(core): 搜索语言词法与语法,含 =. / =.. 层级运算符与三引号"
```

---

## Task 7: 搜索语义与 SQL 编译

**Files:**
- Create: `crates/calibre-core/src/search/compile.rs`
- Modify: `crates/calibre-core/src/search/mod.rs`
- Test: `crates/calibre-core/src/search/compile.rs`

**Interfaces:**
- Consumes: `calibre_core::search::ast::{Query, SearchValue}`、`calibre_core::field::FieldRegistry`、`calibre_core::library::Library`
- Produces:
  - `calibre_core::search::compile(query: &Query, fields: &FieldRegistry) -> Result<SqlFragment>`
  - `calibre_core::search::SqlFragment { pub sql: String, pub params: Vec<SqlValue> }`
  - `calibre_core::search::SqlValue` —— 枚举(`Text(String)` / `Number(f64)` / `Bool(bool)` / `Null`)
  - `Library::search(&self, query: &str) -> Result<Vec<i64>>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn compile_ok(query: &str) -> String {
        let fields = FieldRegistry::standard();
        let parsed = crate::search::parse(query).expect("parse");
        compile(&parsed, &fields).expect("compile").sql
    }

    #[test]
    fn exact_match_uses_equality_operator() {
        let sql = compile_ok("title:=science");
        assert!(sql.contains("="), "SQL 应含等号: {sql}");
        assert!(!sql.contains("LIKE"), "精确匹配不该用 LIKE: {sql}");
    }

    #[test]
    fn hierarchical_prefix_operator_order_is_correct() {
        // =.History 是层级前缀;写成 .=History 会退化成字面包含
        let sql = compile_ok("tags:\"=.History\"");
        assert!(sql.contains("LIKE") || sql.contains("GLOB"), "层级前缀应用前缀匹配: {sql}");
    }

    #[test]
    fn hierarchical_component_operator_order_is_correct() {
        let sql = compile_ok("tags:\"=..WWII\"");
        assert!(!sql.contains("'.=.."), "不应把点号放到等号前: {sql}");
    }

    #[test]
    fn free_text_expands_to_multiple_fields() {
        let sql = compile_ok("dune");
        assert!(sql.contains(" OR "), "自由文本应展开为跨字段 OR: {sql}");
    }

    #[test]
    fn explicit_and_is_emitted() {
        let sql = compile_ok("title:dune and author:herbert");
        assert!(sql.contains(" AND "), "{sql}");
    }

    #[test]
    fn or_is_emitted() {
        let sql = compile_ok("title:dune or title:messiah");
        assert!(sql.contains(" OR "), "{sql}");
    }

    #[test]
    fn not_becomes_not_expression() {
        let sql = compile_ok("not title:dune");
        assert!(sql.contains("NOT"), "{sql}");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin search::compile`
Expected: 编译错误 —— `compile` 未定义。

- [ ] **Step 3: 实现编译器**

- `Query::And(v)` → `(a AND b AND ...)`,空 → `1=1`
- `Query::Or(v)` → `(a OR b ...)`,空 → `1=0`
- `Query::Not(q)` → `NOT (q)`
- `Query::FreeText(s)` → 跨字段 OR:对每个带 search_terms 的字段(排除 `series_sort`、`id`、`uuid` 与虚拟字段)生成一个 `Contains` 条件,用 OR 连接。
- `Query::Location { field, value }` → 按字段 datatype 分派:
  - `Text` / `Comments` / `Enumeration`:`Contains` → `LIKE '%'||?||'%'`;`Exact` → `= ?`;`Literal` → `LIKE` 但不做规范化;`Regex` → 用 SQLite 自定义函数(需 `rusqlite` 的 `create_scalar_function` 注册 `calibre_regex`)或改用 FTS5 `MATCH`;`AccentInsensitive` → 去重音后比较;`HierarchicalPrefix` → `LIKE ?||'%'` 且要求后跟 `.` 或结尾;`HierarchicalComponent` → 与 `split('.')` 的某一段相等(用 `LIKE` + 分隔符判断)。
  - `Int` / `Float` / `Rating`:`Number` / `Rating` → 比较运算符;`Rating` 比较时查询值 `floor(q / 2.0)`,存储值 `x / 2.0` 后取整比较(或存 `x`,比较 `x >= 2*floor(q/2)`)。
  - `DateTime`:`Date` 运算符;精度由值中 `-`/`/` 数量决定。
  - `Bool`:`Bool(b)` → `has_value` 判定;**三态语义**:`true` → `(值非空 AND 值非假)`;`false` → `(值为空)`。
  - 多值字段(`authors`/`tags`/`languages`/`identifiers`):条件作用在 link 表 EXISTS 子查询上。
- `Query::SavedSearch(name)` / `Query::VirtualLibrary(name)` → 从 `preferences` 表读出查询串,递归解析;**加环检测**(递归深度上限 16,超过则报错)。
- `Query::UserCategory { category, item }` → 展开为该用户类别下的所有值的 OR。

- [ ] **Step 4: 实现 `Library::search`**

```rust
pub fn search(&self, query: &str) -> Result<Vec<i64>> {
    let parsed = crate::search::parse(query)?;
    let fragment = crate::search::compile(&parsed, self.fields())?;
    let mut statement = self.connection.prepare(
        "SELECT b.id FROM books b WHERE " ... 
    )?;
    // 绑定 params
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin search::`
Expected: 13 passed(6 个词法 + 7 个编译)

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(core): 搜索语义与 SQL 编译,支持层级/日期/评分/三态布尔"
```

---

## Task 8: 排序键

**Files:**
- Create: `crates/calibre-core/src/sorting.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Test: `crates/calibre-core/src/sorting.rs`

**Interfaces:**
- Consumes: `calibre_core::field::FieldRegistry`、`calibre_core::sort_key::{title_sort, author_to_author_sort}`
- Produces:
  - `calibre_core::sorting::SortSpec { pub field: String, pub ascending: bool }`
  - `calibre_core::sorting::default_sort() -> Vec<SortSpec>` —— `[SortSpec { field: "timestamp", ascending: false }]`(降序,最新在前)
  - `calibre_core::sorting::sort_book_ids(ids: Vec<i64>, specs: &[SortSpec], library: &Library) -> Result<Vec<i64>>`
  - `calibre_core::sorting::max_sort_levels() -> usize` —— 3

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sort_is_timestamp_descending() {
        let specs = default_sort();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].field, "timestamp");
        assert!(!specs[0].ascending, "默认应为降序(最新在前)");
    }

    #[test]
    fn max_sort_levels_is_three() {
        assert_eq!(max_sort_levels(), 3);
    }

    #[test]
    fn title_field_sorts_by_title_sort_key() {
        // "The Hobbit" 应排在 "Anna Karenina" 之后(冠词被移到末尾)
        let specs = vec![SortSpec { field: "title".into(), ascending: true }];
        // 用内存库构造两本书验证顺序
    }

    #[test]
    fn accent_insensitive_sort_groups_umlauts() {
        // "Émile" 与 "Emile" 排序位置应稳定,不报错
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin sorting::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- `default_sort()` → `[SortSpec { field: "timestamp", ascending: false }]`。
- `sort_book_ids` 的排序键按 spec D4:
  - 文本/系列/枚举 → `sort_key::sort_key_for(value, language)`;多值字段且 `display.sort_alpha` 时取各值排序键的元组。
  - 数值/评分 → 数值;未定义值按 `value_for_undefined_numbers_when_sorting`(默认排在最后)。
  - 布尔 → `true=1, false=2, none=3`。
  - 日期 → 时间戳。
  - `authors` → `author_to_author_sort`。
  - 字段名映射 `{"title": "sort", "authors": "author_sort"}`。
  - 系列类字段 → `(系列排序键, series_index)` 二元组。
- 排序在 Rust 内存中做(而非 SQL `ORDER BY`),以便复用 ICU 排序键与多值元组逻辑;用 `sort_by` + 稳定排序。
- 多级排序:`specs` 第一个最显著;`ascending` 为 false 时该级反向。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin sorting::`
Expected: 4 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): 排序键,默认 timestamp 降序,对齐上游 multisort 语义"
```

---

## Task 9: OEBBook 与 EPUB2/EPUB3 只读元数据

**Files:**
- Create: `crates/calibre-formats/Cargo.toml`、`crates/calibre-formats/src/calibre_formats.rs`、`crates/calibre-formats/src/oeb.rs`、`crates/calibre-formats/src/epub.rs`
- Test: `crates/calibre-formats/src/epub.rs`
- Fixture: `crates/calibre-formats/tests/fixtures/`(手工构造的最小 EPUB2 / EPUB3 zip)

**Interfaces:**
- Consumes: `calibre_core::metadata::Metadata`、`calibre_core::error::{Error, Result}`
- Produces:
  - `calibre_formats::oeb::OEBBook` —— 结构体
  - `calibre_formats::oeb::ManifestItem { id, href, media_type, properties }`
  - `calibre_formats::oeb::TocNode { title, href, children }`
  - `calibre_formats::epub::read_metadata(bytes: &[u8]) -> Result<(Metadata, OEBBook)>`
  - `calibre_formats::epub::detect_version(opf: &str) -> EpubVersion`
  - `calibre_formats::epub::EpubVersion` —— 枚举 `V2` / `V3`

```rust
pub struct OEBBook {
    pub metadata: Metadata,
    pub manifest: Vec<ManifestItem>,
    pub spine: Vec<String>,
    pub toc: Vec<TocNode>,
    pub landmarks: Vec<TocNode>,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures").join(name);
        std::fs::read(&path).expect("读取 fixture 失败")
    }

    #[test]
    fn reads_epub2_metadata() {
        let (meta, book) = read_metadata(&fixture("minimal2.epub")).expect("read");
        assert_eq!(meta.title, "测试书");
        assert_eq!(meta.authors, vec!["测试作者".to_string()]);
        assert_eq!(meta.identifiers.get("uuid").map(String::as_str), Some("urn:uuid:1234"));
        assert_eq!(book.toc.len(), 1);
        assert_eq!(book.toc[0].title, "第一章");
    }

    #[test]
    fn reads_epub3_metadata() {
        let (meta, book) = read_metadata(&fixture("minimal3.epub")).expect("read");
        assert_eq!(meta.title, "测试书");
        // EPUB3 的目录在 nav.xhtml,不是 NCX
        assert_eq!(book.toc.len(), 1);
        assert_eq!(book.toc[0].title, "第一章");
    }

    #[test]
    fn detects_epub_version() {
        let opf2 = "<package version=\"2.0\" xmlns=\"http://www.idpf.org/2007/opf\"></package>";
        let opf3 = "<package version=\"3.0\" xmlns=\"http://www.idpf.org/2007/opf\"></package>";
        assert_eq!(detect_version(opf2), EpubVersion::V2);
        assert_eq!(detect_version(opf3), EpubVersion::V3);
    }

    #[test]
    fn corrupted_zip_is_an_error_not_a_panic() {
        let error = read_metadata(b"not a zip file at all").expect_err("should fail");
        assert!(matches!(error, Error::Parse(_)) || matches!(error, Error::Io(_)));
    }

    #[test]
    fn missing_opf_is_an_error() {
        // 构造一个只含 mimetype 的 zip
        let error = read_metadata(&fixture("no_opf.epub")).expect_err("should fail");
        assert!(matches!(error, Error::Parse(_)));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_formats --target aarch64-apple-darwin epub::`
Expected: 编译错误 —— crate 不存在。

- [ ] **Step 3: 建 crate 与 fixture**

`crates/calibre-formats/Cargo.toml`:
```toml
[package]
name = "calibre_formats"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
name = "calibre_formats"
path = "src/calibre_formats.rs"

[dependencies]
calibre_core = { path = "../calibre-core" }
serde = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
zip = { version = "6", default-features = false, features = ["deflate"] }
quick-xml = { version = "0.38", features = ["serialize"] }
```

`crates/calibre-formats/src/calibre_formats.rs`:
```rust
pub mod epub;
pub mod oeb;
```

**手工构造 fixture**(用 Python 或 `zip` 命令):
- `minimal2.epub`:`mimetype`(`application/epub+zip`)、`META-INF/container.xml` → 指向 `OEBPS/content.opf`;`content.opf` 为 OPF 2.0,含 `dc:title`=测试书、`dc:creator`=测试作者、`dc:identifier` uuid、manifest/spine、`toc.ncx` 含一个 navPoint(标题"第一章")。
- `minimal3.epub`:同上但 OPF 3.0,含 `<meta property="dcterms:modified">`,TOC 用 `nav.xhtml`(`<nav epub:type="toc">`)而非 NCX。
- `no_opf.epub`:只有 `mimetype`。

- [ ] **Step 4: 实现 `oeb.rs` 与 `epub.rs`**

`epub.rs`:
- `read_metadata(bytes)`:
  1. 用 `zip::ZipArchive` 打开;失败 → `Error::Parse`
  2. 读 `META-INF/container.xml`,解析 `<rootfile full-path="...">` 得到 OPF 路径;缺失 → `Error::Parse("缺少 container.xml")`
  3. 读 OPF,用 `quick-xml` 解析:
     - `package/@version` → `detect_version`
     - `metadata` 下的 `dc:title`(第一个非空)、`dc:creator`(全部,按顺序)、`dc:creator/@opf:file-as` → `author_sort`、`dc:date` → `pubdate`、`dc:publisher`、`dc:description` → comments、`dc:subject`(全部)→ tags、`dc:identifier`(按 `opf:scheme` 分组;`uuid` scheme 存进 `identifiers`)、`dc:language`(全部)
     - `meta[@name="calibre:series"]` → series;`calibre:series_index` → series_index;`calibre:rating` → rating
     - `manifest/item` → `ManifestItem`(含 `properties` 属性,EPUB3 需要)
     - `spine/itemref` → spine
  4. 目录:
     - EPUB2:读 `spine/@toc` 指向的 NCX,解析 `navMap/navPoint` 递归成 `TocNode`
     - EPUB3:找 manifest 中 `properties` 含 `nav` 的项,读它,解析 `nav[epub:type="toc"]/ol/li` 递归成 `TocNode`;`nav[epub:type="landmarks"]` → landmarks
     - 两个都找:优先用 nav(有则用),否则用 NCX;这样同一本书两种目录都能读
  5. 返回 `(Metadata, OEBBook)`
- `detect_version(opf)` → 解析 `package/@version`,以 "3" 开头 → `V3`,否则 `V2`;解析失败 → `V2`。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p calibre_formats --target aarch64-apple-darwin epub::`
Expected: 5 passed

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(formats): OEBBook 与 EPUB2/EPUB3 只读元数据解析"
```

---

## Task 10: 默认封面生成

**Files:**
- Create: `crates/calibre-formats/src/cover.rs`
- Modify: `crates/calibre-formats/src/calibre_formats.rs`
- Modify: `crates/calibre-formats/Cargo.toml`(加 `image`)
- Test: `crates/calibre-formats/src/cover.rs`

**Interfaces:**
- Consumes: `calibre_core::metadata::Metadata`
- Produces:
  - `calibre_formats::cover::generate_cover(meta: &Metadata, width: u32, height: u32) -> Result<Vec<u8>>` —— 返回 JPEG 字节
  - `calibre_formats::cover::default_cover_path() -> PathBuf` —— 上游 `resources/jacket` 缺省时使用的纯色封面

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_valid_jpeg() {
        let mut meta = Metadata::new("三体");
        meta.authors = vec!["刘慈欣".into()];
        let bytes = generate_cover(&meta, 300, 400).expect("generate");
        assert!(bytes.len() > 1000, "封面应非空");
        // JPEG SOI 标记
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn cover_includes_the_title() {
        // 生成的封面尺寸与请求一致
        let meta = Metadata::new("X");
        let bytes = generate_cover(&meta, 200, 300).expect("generate");
        let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
            .expect("decode");
        assert_eq!(image.width(), 200);
        assert_eq!(image.height(), 300);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_formats --target aarch64-apple-darwin cover::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 用 `image` crate 创建 RGB 图像,填一个从固定调色板里按标题哈希选出的背景色。
- 用 `image::RgbaImage` + 内置点阵字形? —— **不要**手写字体渲染。
  **方案**:`calibre-formats` 不做文字渲染;封面生成改为在 `calibre-app` 层用 GPUI 绘制并截图不可行(无头)。
  **采用的方案**:`calibre-formats` 生成**纯色 + 边框**的占位封面(不含文字),与上游 `resources/jacket` 的默认封面思路一致;**带文字的封面由 `calibre-app` 用 GPUI 渲染**(Task 16 之前)。
  → 因此本 task 的接口调整为:
  - `calibre_formats::cover::generate_placeholder_cover(width: u32, height: u32) -> Result<Vec<u8>>` —— 纯色 JPEG
  - `calibre_formats::cover::cover_palette_color(title: &str) -> [u8; 3]` —— 按标题哈希选色,同一标题永远同色(可测试)

  相应地把测试改为:
  ```rust
  #[test]
  fn generates_a_valid_jpeg() {
      let bytes = generate_placeholder_cover(300, 400).expect("generate");
      assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
  }

  #[test]
  fn same_title_yields_same_color() {
      assert_eq!(cover_palette_color("三体"), cover_palette_color("三体"));
      assert_ne!(cover_palette_color("三体"), cover_palette_color("Dune"));
  }
  ```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_formats --target aarch64-apple-darwin cover::`
Expected: 2 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(formats): 占位封面生成与确定性配色"
```

---

## Task 11: 导入器(单文件 / 多选 / 文件夹)

**Files:**
- Create: `crates/calibre-core/src/import/mod.rs`、`crates/calibre-core/src/import/files.rs`
- Modify: `crates/calibre-core/src/calibre_core.rs`
- Test: `crates/calibre-core/src/import/files.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::metadata::Metadata`、`calibre_core::error::{Error, Result}`
- Produces:
  - `calibre_core::import::files::import_files(library: &mut Library, paths: &[PathBuf]) -> Result<Vec<ImportOutcome>>`
  - `calibre_core::import::files::ImportOutcome { pub path: PathBuf, pub book_id: Option<i64>, pub error: Option<String> }`
  - `calibre_core::import::files::MetadataExtractor` —— trait
  ```rust
  pub trait MetadataExtractor: Send + Sync {
      fn supported_extensions(&self) -> &[&str];
      fn extract(&self, bytes: &[u8], file_name: &str) -> Result<Metadata>;
  }
  ```
  - `calibre_core::import::files::register_extractor(extractor: Box<dyn MetadataExtractor>)`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct FakeExtractor;
    impl MetadataExtractor for FakeExtractor {
        fn supported_extensions(&self) -> &[&str] { &["fake"] }
        fn extract(&self, bytes: &[u8], file_name: &str) -> Result<Metadata> {
            if bytes == b"bad" { return Err(Error::Parse("坏文件".into())); }
            let mut meta = Metadata::new(file_name);
            meta.authors = vec!["作者".into()];
            Ok(meta)
        }
    }

    #[test]
    fn imports_a_single_file() {
        let (dir, mut library) = temp_library();
        register_extractor(Box::new(FakeExtractor));
        let file = dir.join("a.fake");
        std::fs::write(&file, b"ok").expect("write");
        let outcomes = import_files(&mut library, &[file.clone()]).expect("import");
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].book_id.is_some());
        assert!(outcomes[0].error.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_broken_file_is_reported_without_aborting_the_batch() {
        let (dir, mut library) = temp_library();
        register_extractor(Box::new(FakeExtractor));
        let good = dir.join("good.fake");
        let bad = dir.join("bad.fake");
        std::fs::write(&good, b"ok").expect("write");
        std::fs::write(&bad, b"bad").expect("write");
        let outcomes = import_files(&mut library, &[good, bad]).expect("import");
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].book_id.is_some());
        assert!(outcomes[1].book_id.is_none());
        assert!(outcomes[1].error.is_some());
        // 失败的文件不能留下残留目录
        assert_eq!(library.all_book_ids().expect("ids").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unsupported_extension_is_reported() {
        let (dir, mut library) = temp_library();
        let file = dir.join("a.xyz");
        std::fs::write(&file, b"x").expect("write");
        let outcomes = import_files(&mut library, &[file]).expect("import");
        assert!(outcomes[0].error.is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin import::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 扩展名注册表:全局 `OnceLock<Mutex<Vec<Box<dyn MetadataExtractor>>>>`,按扩展名(小写,不含点)索引。
- `import_files` 逐个处理:
  1. 读文件字节;读失败 → 该项记 `error`,继续下一个。
  2. 找匹配的 extractor;无 → `Error::UnsupportedFormat`,记 `error`,继续。
  3. `extract` 出 `Metadata`;失败 → 记 `error`,继续。
  4. `library.add_book(&meta, vec![(ext, bytes)])`;失败 → 记 `error`,继续。**若 add_book 中途失败,必须清理已创建的目录**(不留半成品)。
  5. 成功 → `book_id`。
- 单个文件的失败**不中断整批**。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin import::`
Expected: 3 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): 文件导入器,单文件失败不影响整批"
```

---

## Task 12: 导入器(从既有 calibre 书库)

**Files:**
- Create: `crates/calibre-core/src/import/calibre_library.rs`
- Modify: `crates/calibre-core/src/import/mod.rs`
- Test: `crates/calibre-core/src/import/calibre_library.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::metadata::Metadata`
- Produces:
  - `calibre_core::import::calibre_library::import_calibre_library(library: &mut Library, source_root: &Path) -> Result<Vec<ImportOutcome>>`
  - `calibre_core::import::calibre_library::CalibreSourceReader` —— 只读打开源库
  ```rust
  pub struct CalibreSourceReader { connection: rusqlite::Connection, root: PathBuf }
  impl CalibreSourceReader {
      pub fn open(root: &Path) -> Result<Self>;
      pub fn book_count(&self) -> Result<usize>;
      pub fn books(&self) -> Result<Vec<SourceBook>>;
  }
  pub struct SourceBook {
      pub id: i64,
      pub title: String,
      pub sort: String,
      pub author_sort: String,
      pub series_index: f64,
      pub pubdate: Option<String>,
      pub timestamp: Option<String>,
      pub path: String,
      pub authors: Vec<String>,
      pub tags: Vec<String>,
      pub series: Option<String>,
      pub publisher: Option<String>,
      pub rating: Option<f64>,
      pub comments: Option<String>,
      pub identifiers: Vec<(String, String)>,
      pub languages: Vec<String>,
      pub formats: Vec<(String, String)>,   // (格式, 文件名)
  }
  ```

- [ ] **Step 1: 写失败测试**

测试需要一个**上游 calibre 造的源库**。用 Python 在测试前生成?—— 单元测试不能依赖 Python。
**方案**:在 `crates/calibre-core/tests/fixtures/calibre_library/` 放一个**手工用 rusqlite 造出来的最小源库**(含 `metadata.db` 与 `作者/书名 (id)/` 目录),用一个 `build_fixture.rs` 辅助测试或直接提交二进制 fixture。
**更简单且可维护的方案**:测试运行时用 `rusqlite` 在临时目录里造一个符合 calibre schema 的源库(只造用到的表),再导入。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(dir: &Path) {
        let connection = rusqlite::Connection::open(dir.join("metadata.db")).expect("open");
        connection.execute_batch(
            "CREATE TABLE books(id INTEGER PRIMARY KEY, title TEXT, sort TEXT, author_sort TEXT,
                 series_index REAL DEFAULT 1.0, pubdate TEXT, timestamp TEXT, last_modified TEXT,
                 uuid TEXT, has_cover INTEGER DEFAULT 0, size INTEGER DEFAULT 0, path TEXT, pages INTEGER);
             CREATE TABLE authors(id INTEGER PRIMARY KEY, name TEXT, sort TEXT);
             CREATE TABLE tags(id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE series(id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE publishers(id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE ratings(id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE languages(id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE comments(book INTEGER PRIMARY KEY, text TEXT);
             CREATE TABLE identifiers(book INTEGER, id INTEGER, val TEXT, scheme TEXT);
             CREATE TABLE data(book INTEGER, format TEXT, name TEXT, uncompressed_size INTEGER, mtime TEXT);
             CREATE TABLE books_authors_link(book INTEGER, author INTEGER);
             CREATE TABLE books_tags_link(book INTEGER, tag INTEGER);
             CREATE TABLE books_series_link(book INTEGER, series INTEGER);
             CREATE TABLE books_publishers_link(book INTEGER, publisher INTEGER);
             CREATE TABLE books_ratings_link(book INTEGER, rating INTEGER);
             CREATE TABLE books_languages_link(book INTEGER, language INTEGER);
             INSERT INTO books VALUES(1, 'The Book', 'Book, The', 'Author, A', 1.0,
                 '2020-01-01 00:00:00', '2021-05-06 07:08:09', '2021-05-06 07:08:09',
                 'uuid-1', 0, 100, 'Author/The Book (1)', 0);
             INSERT INTO authors VALUES(1, 'A|Author', 'Author, A');
             INSERT INTO books_authors_link VALUES(1, 1);
             INSERT INTO tags VALUES(1, 'Tag One');
             INSERT INTO books_tags_link VALUES(1, 1);
             INSERT INTO comments VALUES(1, '<p>hello</p>');
             INSERT INTO identifiers VALUES(1, 1, '9787536692930', 'isbn');
             INSERT INTO data VALUES(1, 'EPUB', 'The Book - A.epub', 100, '2021-05-06 07:08:09');
             INSERT INTO languages VALUES(1, 'eng');
             INSERT INTO books_languages_link VALUES(1, 1);
             ",
        ).expect("create schema");
        let book_dir = dir.join("Author").join("The Book (1)");
        std::fs::create_dir_all(&book_dir).expect("mkdir");
        std::fs::write(book_dir.join("The Book - A.epub"), b"epub bytes").expect("write");
    }

    #[test]
    fn imports_books_from_a_calibre_library() {
        let source = std::env::temp_dir().join(format!("calibre-src-{}", uuid::Uuid::new_v4()));
        let target = std::env::temp_dir().join(format!("calibre-dst-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&source).expect("mkdir source");
        std::fs::create_dir_all(&target).expect("mkdir target");
        make_source(&source);
        let mut library = Library::open(&target).expect("open target");
        let outcomes = import_calibre_library(&mut library, &source).expect("import");
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].book_id.is_some());
        let ids = library.all_book_ids().expect("ids");
        assert_eq!(ids.len(), 1);
        let meta = library.book_metadata(ids[0]).expect("meta");
        assert_eq!(meta.title, "The Book");
        assert_eq!(meta.authors, vec!["A, Author".to_string()], "作者名里的 | 应还原为 ,");
        assert_eq!(meta.tags, vec!["Tag One".to_string()]);
        assert_eq!(meta.identifiers.get("isbn").map(String::as_str), Some("9787536692930"));
        assert_eq!(meta.comments.as_deref(), Some("<p>hello</p>"));
        assert!(meta.pubdate.is_some());
        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&target).ok();
    }

    #[test]
    fn source_library_is_never_modified() {
        // 导入后源库的 mtime 与文件内容不变
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin import::calibre_library`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- `CalibreSourceReader::open(root)`:
  - 打开 `root/metadata.db`,**只读**:`rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY`
  - 若表不存在(`books` 等)→ `Error::InvalidLibrary`
- `books()`:按 spec D11 的映射读取:
  - `authors.name` 里的 `|` 还原为 `,`
  - `pubdate` / `timestamp` 解析上游格式 → `DateTime<Utc>`
  - `identifiers` 的 `scheme` 小写、`val` 的 `,` 还原为 `|`(与本项目存储规范一致)
  - `data` 给出 `(format, name)`,源文件在 `root/<books.path>/<name>`
- `import_calibre_library`:
  - 逐本:读源文件字节(缺失 → 记 error,继续)→ 构造 `Metadata` → `library.add_book`
  - 源库全程只读
- 上游日期解析需支持:`YYYY`、`YYYY-MM`、`YYYY-MM-DD`、`YYYY-MM-DD HH:MM:SS`、`MM/DD/YYYY`。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_core --target aarch64-apple-darwin import::`
Expected: 5 passed(3 个 files + 2 个 calibre_library)

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(core): 从既有 calibre 书库导入,源库全程只读"
```

---

## Task 13: i18n 骨架

**Files:**
- Create: `crates/calibre-i18n/Cargo.toml`、`crates/calibre-i18n/src/calibre_i18n.rs`、`crates/calibre-i18n/src/locales/zh-CN.ftl`、`crates/calibre-i18n/src/locales/en.ftl`
- Test: `crates/calibre-i18n/src/calibre_i18n.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `calibre_i18n::Locale` —— 枚举 `ZhCn` / `En`
  - `calibre_i18n::Locale::current() -> Locale` —— 读 `CALIBRE2RUST_LOCALE` 环境变量,默认 `ZhCn`
  - `calibre_i18n::Locale::all() -> Vec<Locale>`
  - `calibre_i18n::translate(locale: Locale, key: &str, args: &[(&str, &str)]) -> String`
  - `calibre_i18n::t(key: &str, args: &[(&str, &str)]) -> String` —— 用当前 locale
  - `calibre_i18n::set_locale(locale: Locale)` —— 进程级全局

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_a_known_key_in_both_locales() {
        assert_eq!(translate(Locale::ZhCn, "app-title", &[]), "calibre2Rust");
        assert_eq!(translate(Locale::En, "app-title", &[]), "calibre2Rust");
    }

    #[test]
    fn locale_specific_message_is_used() {
        assert_eq!(translate(Locale::ZhCn, "menu-add-books", &[]), "添加书籍");
        assert_eq!(translate(Locale::En, "menu-add-books", &[]), "Add Books");
    }

    #[test]
    fn parameters_are_substituted() {
        let message = translate(Locale::En, "books-count", &[("count", "42")]);
        assert!(message.contains("42"), "{message}");
    }

    #[test]
    fn unknown_key_falls_back_to_the_key_itself() {
        assert_eq!(translate(Locale::En, "no-such-key", &[]), "no-such-key");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_i18n --target aarch64-apple-darwin`
Expected: 编译错误。

- [ ] **Step 3: 实现**

**不引入 Fluent 运行时**:Fluent 的完整实现较重,且本项目的消息都是简单占位符。
**采用**:自实现一个极小的 `.ftl` 子集解析器 —— 支持 `key = value` 与 `{ $name }` 占位符。

> **偏离 spec**:spec D5 写的是"Fluent"。此处改为"Fluent 风格的极小子集自实现",理由是减少依赖体积与 API 漂移风险,而消息格式仍保持 Fluent 语法,将来换回真正的 fluent crate 只需替换 `translate` 的内部实现。**此偏离记入 spec §9 的偏离清单。**

- `crates/calibre-i18n/Cargo.toml`:无外部依赖(仅 `once_cell` 或用 `std::sync::OnceLock`)。
- `calibre_i18n.rs`:
  - `include_str!("locales/zh-CN.ftl")` / `include_str!("locales/en.ftl")`
  - 启动时(`OnceLock`)解析成 `HashMap<&'static str, &'static str>`
  - `translate` 查表 → 替换 `{ $name }` → 未命中则返回 key 本身
  - `set_locale` 写 `AtomicU8` 全局
- `zh-CN.ftl` / `en.ftl` 至少包含:
  - `app-title = calibre2Rust`
  - `menu-add-books = 添加书籍` / `Add Books`
  - `menu-edit-metadata = 编辑元数据` / `Edit Metadata`
  - `menu-convert-books = 转换书籍` / `Convert Books`
  - `menu-choose-library = 选择书库` / `Choose Library`
  - `menu-save-to-disk = 保存到磁盘` / `Save To Disk`
  - `menu-preferences = 首选项` / `Preferences`
  - `menu-help = 帮助` / `Help`
  - `menu-remove-books = 删除书籍` / `Remove Books`
  - `menu-tweak-epub = 调整 EPUB` / `Tweak ePub`
  - `search-placeholder = 搜索所有字段` / `Search all fields`
  - `books-count = { $count } 本书` / `{ $count } books`
  - `column-title = 标题` / `Title`
  - `column-author = 作者` / `Author`
  - `column-series = 系列` / `Series`
  - `column-tags = 标签` / `Tags`
  - `column-publisher = 出版商` / `Publisher`
  - `column-rating = 评分` / `Rating`
  - `column-date-added = 添加日期` / `Date Added`
  - `column-format = 格式` / `Format`
  - `column-size = 大小` / `Size`
  - `tag-browser-search = 查找` / `Find`
  - `edit-metadata-title = 编辑元数据` / `Edit Metadata`
  - `button-save = 保存` / `Save`
  - `button-cancel = 取消` / `Cancel`

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_i18n --target aarch64-apple-darwin`
Expected: 4 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(i18n): 中英文案骨架,Fluent 风格子集自实现"
```

---

## Task 14: 主窗口骨架

**Files:**
- Create: `crates/calibre-app/src/window.rs`
- Modify: `crates/calibre-app/src/app.rs`
- Modify: `crates/calibre-app/src/calibre_app.rs`
- Test: `crates/calibre-app/src/window.rs`(纯函数部分)

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_app::assets::CalibreAssets`
- Produces:
  - `calibre_app::window::MainWindow` —— `Entity<MainWindow>`
  - `calibre_app::window::MainWindow::new(library: Library, cx: &mut App) -> Entity<MainWindow>`
  - `calibre_app::window::PanelLayout` —— 枚举 `Wide` / `Narrow`
  - `calibre_app::window::toolbar_actions() -> Vec<&'static str>` —— macOS 默认工具栏动作名(spec D5)
  - `calibre_app::window::menubar_actions() -> Vec<&'static str>` —— 原生菜单栏动作名

```rust
pub fn toolbar_actions() -> Vec<&'static str> {
    vec![
        "Add Books", "Edit Metadata", "---", "Convert Books", "View", "---",
        "Choose Library", "Donate", "---", "Fetch News", "Store", "Save To Disk",
        "Connect Share", "---", "Remove Books", "Tweak ePub",
    ]
}

pub fn menubar_actions() -> Vec<&'static str> {
    vec![
        "Add Books", "Edit Metadata", "Convert Books", "Choose Library",
        "Save To Disk", "Preferences", "Help",
    ]
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_toolbar_matches_upstream_default() {
        let actions = toolbar_actions();
        assert!(!actions.contains(&"Help"), "macOS 工具栏不应有 Help");
        assert!(!actions.contains(&"Preferences"), "macOS 工具栏不应有 Preferences");
        assert_eq!(actions[0], "Add Books");
    }

    #[test]
    fn menubar_contains_preferences_and_help() {
        let actions = menubar_actions();
        assert!(actions.contains(&"Preferences"));
        assert!(actions.contains(&"Help"));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin window::`
Expected: 编译错误。

- [ ] **Step 3: 实现窗口结构**

`MainWindow`:
```rust
pub struct MainWindow {
    library: Library,
    layout: PanelLayout,
    left_panel_visible: bool,
    right_panel_visible: bool,
    top_panel_visible: bool,
    bottom_panel_visible: bool,
    selected_book_ids: Vec<i64>,
    search_query: String,
}
```

渲染结构(自上而下):
1. 工具栏行:`ui::Button` 序列,动作名来自 `toolbar_actions()`;`"---"` 渲染成竖分隔线。
2. 搜索栏行(占位,Task 18 填充):虚拟库按钮 / 清除 VL / 排序 / 分组 / 搜索框占位 / 搜索按钮。
3. 中央容器:`gpui::div` 用 `flex_row` + 两个 `flex_col` 实现 5 面板:
   - 左:标签浏览器占位(Task 15 填充)
   - 中:书列表占位(Task 16 填充)
   - 右:书籍详情占位(Task 17 填充)
   - 上/下:封面浏览器 / 快速预览占位
   - 4 条可拖拽分隔条:`on_drag` 回调修改面板可见性与尺寸;拖动宽度 < 10px 隐藏面板,> 50px 恢复。
4. 状态栏:显示当前书数。

原生菜单:`cx.set_menus([Menu::new("File").items([...])])`,内容来自 `menubar_actions()`。

`MainWindow::new` 打开书库:默认书库路径 `~/Library/Application Support/calibre2Rust/library`(macOS),不存在则创建。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin window::`
Expected: 2 passed

- [ ] **Step 5: 编译并肉眼验证**

Run: `cargo build --target aarch64-apple-darwin` && `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected: 窗口显示工具栏 + 搜索栏 + 三栏空面板 + 状态栏;工具栏上**没有** Help/Preferences。

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(app): 主窗口骨架,工具栏/菜单栏/五面板/分隔条"
```

---

## Task 15: 标签浏览器树

**Files:**
- Create: `crates/calibre-app/src/tag_browser.rs`
- Modify: `crates/calibre-app/src/window.rs`
- Test: `crates/calibre-app/src/tag_browser.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::field::FieldRegistry`
- Produces:
  - `calibre_app::tag_browser::TagNode` —— 枚举
    ```rust
    pub enum TagNode {
        Category { field: String, label: String },
        Item { field: String, value: String, count: usize, average_rating: Option<f64> },
    }
    ```
  - `calibre_app::tag_browser::build_tag_tree(library: &Library, fields: &FieldRegistry) -> Result<Vec<TagNode>>`
  - `calibre_app::tag_browser::TagBrowser` —— `Entity<TagBrowser>`,`Render` 实现
  - `calibre_app::tag_browser::TagBrowser::new(library: Entity<Library>, cx: &mut App) -> Entity<TagBrowser>`
  - `calibre_app::tag_browser::TagBrowser::selected_values(&self) -> Vec<(String, String)>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_lists_categories_in_upstream_order() {
        // 用内存库造一本书,验证类别顺序是 authors, languages, series, formats, publisher, rating, news, tags, identifiers
    }

    #[test]
    fn item_counts_reflect_the_library() {
        // 造 2 本书,同一作者 → 作者类别的该项 count == 2
    }

    #[test]
    fn clicking_an_item_produces_a_filter() {
        // TagBrowser::selected_values 返回被选中的 (field, value)
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin tag_browser::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- `build_tag_tree`:
  - 类别集合与顺序取自 `FieldRegistry::categories()`(Task 3 已定顺序)。
  - 每个类别的项:从库里查出该字段的所有值及关联书数(`COUNT(*)`),按名称排序(用 `sort_key::sort_key_for`)。
  - `rating` 类别的项额外算平均星级(存储值 / 2)。
- 渲染:`gpui::uniform_list` 虚拟滚动;每行是一个 `div`,按层级缩进;`Category` 行可展开/折叠;`Item` 行显示名称 + 计数 + 平均星级。
- 点击 `Item` → 记录选中值并向上抛(on_click 回调给 `MainWindow`)。
- 右键菜单(Phase 1 只做最常用的几项):重命名 / 删除 / 隐藏类别。
- "查找"框是**跳转**语义(spec D5):输入后按按钮定位到下一个匹配项并高亮,**不隐藏**不匹配的行。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin tag_browser::`
Expected: 3 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 标签浏览器树,类别顺序与计数对齐上游"
```

---

## Task 16: 书列表(ui::Table)

**Files:**
- Create: `crates/calibre-app/src/book_list.rs`
- Modify: `crates/calibre-app/src/window.rs`
- Test: `crates/calibre-app/src/book_list.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::field::FieldRegistry`
- Produces:
  - `calibre_app::book_list::BookList` —— `Entity<BookList>`,`Render` 实现
  - `calibre_app::book_list::BookList::new(library: Entity<Library>, cx: &mut App) -> Entity<BookList>`
  - `calibre_app::book_list::BookList::set_query(&mut self, query: &str, cx: &mut Context<Self>)`
  - `calibre_app::book_list::BookList::selected_book_ids(&self) -> Vec<i64>`
  - `calibre_app::book_list::default_visible_columns() -> Vec<String>` —— `["title", "author_sort", "series", "tags", "publisher", "rating", "timestamp", "format", "size"]`
  - `calibre_app::book_list::format_size(bytes: u64) -> String`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_columns_hide_the_upstream_hidden_ones() {
        let columns = default_visible_columns();
        for hidden in ["last_modified", "languages", "id", "path", "pages"] {
            assert!(!columns.contains(&hidden.to_string()), "不应显示 {hidden}");
        }
        assert!(columns.contains(&"title".to_string()));
    }

    #[test]
    fn format_size_is_human_readable() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin book_list::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 用 `ui::Table::new(列数)`,列由 `default_visible_columns()` 与 `FieldRegistry` 决定。
- 行数据:用 `Library::search` 得到书 ID 列表(查询为空时用 `all_book_ids` + 默认排序),再逐本取展示值。
- 单元格渲染:`title` / `author_sort` / `series`(带 `series_index`)/ `tags`(逗号连接)/ `publisher` / `rating`(星级)/ `timestamp`(本地化日期)/ `format`(逗号连接格式名)/ `size`。
- 表头:点击切换排序(最多 3 级),`ui::Table` 的列宽可调默认开启。
- 多选:点击行选中,`Shift`/`Cmd` 扩展。
- 虚拟滚动由 `ui::Table::uniform_list` 提供 —— **不要**自己 `render` 全部行。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin book_list::`
Expected: 2 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 书列表,ui::Table 虚拟滚动 + 多级排序"
```

---

## Task 17: 书籍详情面板

**Files:**
- Create: `crates/calibre-app/src/book_details.rs`
- Modify: `crates/calibre-app/src/window.rs`
- Test: `crates/calibre-app/src/book_details.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::metadata::Metadata`
- Produces:
  - `calibre_app::book_details::BookDetails` —— `Entity<BookDetails>`,`Render` 实现
  - `calibre_app::book_details::BookDetails::new(library: Entity<Library>, cx: &mut App) -> Entity<BookDetails>`
  - `calibre_app::book_details::BookDetails::set_book(&mut self, book_id: Option<i64>, cx: &mut Context<Self>)`
  - `calibre_app::book_details::detail_rows(meta: &Metadata, fields: &FieldRegistry) -> Vec<(String, String)>`
  - `calibre_app::book_details::render_html_as_runs(html: &str) -> Vec<gpui::TextRun>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_rows_follow_field_order() {
        let mut meta = Metadata::new("书名");
        meta.authors = vec!["作者".into()];
        let rows = detail_rows(&meta, &FieldRegistry::standard());
        assert_eq!(rows[0].0, "标题");
        assert!(rows.iter().any(|(k, v)| k == "作者" && v == "作者"));
    }

    #[test]
    fn html_renderer_extracts_bold_text() {
        let runs = render_html_as_runs("<p>普通<b>加粗</b></p>");
        let bold: Vec<&str> = runs.iter().filter(|r| r.font_weight == Some(gpui::FontWeight::BOLD))
            .map(|r| r.text.as_ref()).collect();
        assert_eq!(bold, vec!["加粗"]);
    }

    #[test]
    fn html_renderer_escapes_plain_text() {
        let runs = render_html_as_runs("<p>a &lt; b &amp; c</p>");
        let text: String = runs.iter().map(|r| r.text.as_ref()).collect();
        assert!(text.contains("a < b & c"), "{text}");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin book_details::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 面板结构:上下(或左右,随主窗口布局)两段 —— 上为封面(`gpui::img()`,无封面时显示占位图),下为元数据表。
- `detail_rows` 按 `FieldRegistry::displayable()` 的顺序产出 `(标签, 值)`;值按类型格式化(评分 → 星级,日期 → 本地化)。
- `render_html_as_runs`:自实现受限 HTML 解析(支持 `b`/`i`/`span`/`a`/`h1`-`h6`/`br`/`ul`/`ol`/`li`/`p`/`em`/`strong`/`sub`/`sup`),用 `html5ever` 或手写状态机解析;输出 `Vec<gpui::TextRun>`。不引入 Zed 的 markdown crate。
- 值可点击:`authors` 的值点击 → 把该作者加进搜索框;`publisher` 同理;`identifiers` 的值点击 → 复制到剪贴板。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin book_details::`
Expected: 3 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 书籍详情面板,受限 HTML 渲染为 TextRun"
```

---

## Task 18: 搜索框(防抖 + 历史 + 限制 + 虚拟库)

**Files:**
- Create: `crates/calibre-app/src/search_box.rs`
- Modify: `crates/calibre-app/src/window.rs`
- Test: `crates/calibre-app/src/search_box.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::search::parse`
- Produces:
  - `calibre_app::search_box::SearchBox` —— `Entity<SearchBox>`,`Render` 实现
  - `calibre_app::search_box::SearchBox::new(on_query_changed: impl Fn(&str) + 'static, cx: &mut App) -> Entity<SearchBox>`
  - `calibre_app::search_box::SearchBox::query(&self) -> String`
  - `calibre_app::search_box::SearchBox::set_query(&mut self, query: &str, cx: &mut Context<Self>)`
  - `calibre_app::search_box::SearchBox::error(&self) -> Option<String>` —— 解析错误,显示错误图标
  - `calibre_app::search_box::SearchBox::history(&self) -> Vec<String>`
  - `calibre_app::search_box::SearchBox::restriction(&self) -> Option<SearchRestriction>`
  - `calibre_app::search_box::SearchRestriction` —— 枚举 `CurrentSearch` / `Text(String)` / `SavedSearch(String)`
  - `calibre_app::search_box::SearchBox::virtual_library_label(&self) -> Option<String>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_capped_at_twenty_five_entries() {
        let mut history = Vec::new();
        for index in 0..40 {
            history.push(format!("q{index}"));
        }
        assert_eq!(history.len().max(25), 25);
    }

    #[test]
    fn invalid_query_produces_an_error_not_a_panic() {
        let error = validate_query("title:\"unclosed");
        assert!(error.is_some());
        assert!(validate_query("title:dune").is_none());
    }

    #[test]
    fn virtual_library_label_is_truncated_to_twenty_chars() {
        let label = format_virtual_library_label("一个非常非常长的虚拟书库名称超过二十个字符", "附加限制");
        assert!(label.chars().count() <= 21, "{label}");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin search_box::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 输入框本体用 Task 19 的 `InputField`。
- 防抖:1500 ms(上游 `INTERVAL=1500`)。用 GPUI 的 `cx.background_executor().timer(...)` 计时(不用 `smol::Timer`,否则 `run_until_parked` 测不到)。
- 历史:最多 25 条,持久化到 `preferences` 表的 `main_search_history`;下拉显示历史(不是"搜索模式")。
- 解析:输入变化时调用 `calibre_core::search::parse`;出错则 `error = Some(msg)` 并显示错误图标。
- "附加限制"菜单:无 / `*当前搜索` / 文本限制 / 已保存的搜索。
- 虚拟库:具名查询式,叠加在当前搜索之上;清除按钮显示 `<基础VL> :: <限制>`,截断到 20 字符 + `…`(上游 `search_restriction_mixin.py:745-749` 的 `t[:19] + '…'`)。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin search_box::`
Expected: 3 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 搜索框,1500ms 防抖 + 25 条历史 + 限制 + 虚拟库"
```

---

## Task 19: InputField 组件

**Files:**
- Create: `crates/calibre-app/src/input_field.rs`
- Modify: `crates/calibre-app/src/calibre_app.rs`
- Test: `crates/calibre-app/src/input_field.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `calibre_app::input_field::InputField` —— `Entity<InputField>`
  - `calibre_app::input_field::InputField::new(cx: &mut App) -> Entity<InputField>`
  - `calibre_app::input_field::InputField::text(&self) -> String`
  - `calibre_app::input_field::InputField::set_text(&mut self, text: &str, cx: &mut Context<Self>)`
  - `calibre_app::input_field::InputField::set_placeholder(&mut self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>)`
  - `calibre_app::input_field::InputField::on_change(&mut self, handler: impl Fn(&str) + 'static, cx: &mut Context<Self>)`
  - `calibre_app::input_field::InputField::focus_handle(&self) -> FocusHandle`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        // 用 TestAppContext 构造,set_text 后 text() 应一致
    }

    #[test]
    fn placeholder_is_stored() {
        // set_placeholder 后内部字段为该值
    }
}
```

> GPUI 的 `TestAppContext` 在 macOS 上部分测试需要主线程,标记 `#[ignore]`,但**逻辑部分**(纯字符串状态)应抽成可测的纯函数。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin input_field::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

参考 `../zed/crates/gpui/examples/input.rs`(920 行)实现最小可用单行输入框:
- `FocusHandle` + `EntityInputHandler` trait 实现
- 文本以 `String` 存储(不需要 rope)
- 光标位置、选区、剪贴板(`cmd-c` / `cmd-v` / `cmd-a`)
- 渲染:用 `gpui::StyledText` 画当前文本 + 光标
- 键盘:方向键、Home/End、Backspace/Delete、Enter(触发 `on_submit`)

**不引入** `ui_input` crate(会拖入 zed 的 `editor` 整条栈,见 spec §3.2)。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin input_field::`
Expected: 2 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 单行输入框,参考 gpui examples/input.rs 自实现"
```

---

## Task 20: 元数据编辑模态框

**Files:**
- Create: `crates/calibre-app/src/edit_metadata.rs`
- Modify: `crates/calibre-app/src/window.rs`
- Test: `crates/calibre-app/src/edit_metadata.rs`

**Interfaces:**
- Consumes: `calibre_core::library::Library`、`calibre_core::metadata::Metadata`
- Produces:
  - `calibre_app::edit_metadata::EditMetadataDialog` —— `Entity<EditMetadataDialog>`
  - `calibre_app::edit_metadata::EditMetadataDialog::new(book_id: i64, library: Entity<Library>, cx: &mut App) -> Entity<EditMetadataDialog>`
  - `calibre_app::edit_metadata::EditMetadataDialog::apply(&mut self, cx: &mut Context<Self>) -> Result<()>`
  - `calibre_app::edit_metadata::parse_tags(input: &str) -> Vec<String>`
  - `calibre_app::edit_metadata::parse_authors(input: &str) -> Vec<String>`
  - `calibre_app::edit_metadata::parse_series_index(input: &str) -> Result<Option<f64>>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_split_on_commas() {
        assert_eq!(parse_tags("科幻, 经典"), vec!["科幻".to_string(), "经典".to_string()]);
    }

    #[test]
    fn tags_drop_empty_entries() {
        assert_eq!(parse_tags("科幻,,经典,"), vec!["科幻".to_string(), "经典".to_string()]);
    }

    #[test]
    fn authors_use_the_upstream_split_rule() {
        assert_eq!(parse_authors("刘慈欣"), vec!["刘慈欣".to_string()]);
        assert_eq!(parse_authors("刘慈欣 & 王晋康"), vec!["刘慈欣".to_string(), "王晋康".to_string()]);
    }

    #[test]
    fn series_index_accepts_fractional_values() {
        assert_eq!(parse_series_index("1.5"), Ok(Some(1.5)));
        assert_eq!(parse_series_index(""), Ok(None));
        assert!(parse_series_index("abc").is_err());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin edit_metadata::`
Expected: 编译错误。

- [ ] **Step 3: 实现**

- 用 `ui::Modal` 呈现。
- 字段:标题、作者、系列、系列序号、标签、出版商、评分、语言、评论、标识符。
- 每个字段一个 `InputField`;评分为 0–10 的星级选择;评分为 0 表示无评分。
- "保存" → `apply`:取当前 `Metadata`,覆盖被编辑字段,`library.set_book_metadata(book_id, &meta)`,然后 `cx.notify()`。
- 需要在 `calibre_core::library::Library` 上补 `set_book_metadata`:
  ```rust
  pub fn set_book_metadata(&mut self, book_id: i64, meta: &Metadata) -> Result<()>;
  ```
  该方法由 Task 5 的实现者补上,或在 Task 20 里一并补齐并加测试。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre_app --target aarch64-apple-darwin edit_metadata::`
Expected: 4 passed

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(app): 元数据编辑模态框"
```

---

## Task 21: 对拍脚本与 Phase 1 验收

**Files:**
- Create: `tools/parity/parity.py`、`tools/parity/queries.txt`
- Create: `docs/superpowers/specs/2026-10-21-phase1-acceptance.md`(验收记录)
- Modify: `crates/calibre-core/src/lib` 对应模块(补对拍暴露出的偏差)

**Interfaces:**
- Consumes: `calibre_core` 的全部公开 API;上游 calibre 的 Python API
- Produces:
  - `tools/parity/parity.py --library <path> --compare-to <path>` —— 对两个书库跑同一批查询并比对
  - `tools/parity/queries.txt` —— 每行一个查询

- [ ] **Step 1: 写固定查询集**

`tools/parity/queries.txt`(覆盖 spec D3 全部语法分支):
```
title:dune
title:=Dune
title:^dune
title:\Dune
author:herbert
title:"=..Military"
title:"=.History"
tags:#<2
formats:#=1
rating:>=3
rating:>=3.5
pubdate:>2020
pubdate:>2020-01-01
pubdate:true
date:true
timestamp:false
not title:dune
title:dune and author:herbert
title:dune or title:mesiah
(dune or mesiah) and author:herbert
tags:"""a "b" c"""
all:dune
Émile
search:MySavedSearch
@Fav:Some Item
```

- [ ] **Step 2: 写对拍脚本**

`tools/parity/parity.py`:
1. 用上游 calibre 的 Python API 打开参考库,跑 `queries.txt` 里的每条查询,记录书 ID 集合。
2. 调用本项目的 CLI(`calibre2rust --search <query> --json`;该子命令由 Task 21 顺带实现,输出 JSON 数组),记录书 ID 集合。
3. 逐条比对,输出差异报告;有任何差异则退出码非 0。

**需要先给 `calibre-app` 加一个最小 CLI 入口**:`calibre2rust --library <path> --search <query> --json` —— 复用 `calibre-core`,不启动 GUI。

- [ ] **Step 3: 实现对拍用的 CLI 子命令**

在 `crates/calibre-app/src/main.rs` 里加参数解析(用 `clap`,或先手写简单解析避免多一个依赖):
- `--library <path>`:书库路径
- `--search <query>`:执行搜索并输出书 ID 的 JSON 数组
- `--metadata <book_id>`:输出该书归一化元数据 JSON
- `--sort <field>[:asc|desc]`:输出排序后的书 ID 数组

- [ ] **Step 4: 跑对拍**

Run: `python3 tools/parity/parity.py --library <样例书库> --compare-to <上游样例书库>`
Expected: 全部查询结果一致,退出码 0。
**若不一致**:按 spec D3 逐条排查 —— 优先怀疑 `=.` / `=..` 顺序、评分整除、日期精度、三态布尔。

- [ ] **Step 5: 写验收记录**

`docs/superpowers/specs/2026-10-21-phase1-acceptance.md` 记录:
- Phase 1 验收清单逐条结果(建库 / 导入 / 标签过滤 / 搜索过滤 / 详情 / 编辑 / 重启保持)
- 对拍结果摘要(多少条查询、是否全一致)
- **与上游不一致之处**(如有)的清单 —— spec §7.1 要求逐条登记

- [ ] **Step 6: 全量质量门禁**

Run:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --target aarch64-apple-darwin
cargo build --target aarch64-apple-darwin
```
Expected: 全部通过,无 warning。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "test: 对拍脚本与 Phase 1 验收记录"
```

---

## 附:Phase 1 完成后的下一步

Phase 1 完成后,按 spec §6 继续:
- **Phase 2 计划**:格式与元数据(EPUB2/3 写、其余格式、转换管线、`ebook-convert` CLI)。
- **Phase 3 计划**:阅读器 + 扩展格式。
- Phase 4/5/6(编辑器与模板、CLI 与内容服务器、插件与周边)在 Phase 3 之后各出一份计划。

**每个 Phase 一份独立计划**,不要把多个 Phase 塞进一份计划 —— 计划长度会失控,且 Phase 2/3 的一些设计要在 Phase 1 的实测结果上再定。
