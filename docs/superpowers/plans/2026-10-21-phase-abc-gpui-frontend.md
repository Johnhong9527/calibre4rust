# Phase A+B+C 实施计划:依赖打通、主窗口骨架、书列表

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个能打开 caliberate 书库、以表格视图浏览并排序全部书籍的 GPUI 桌面应用,主窗口布局对齐 calibre。

**Architecture:** 单 crate `calibre2rust-gui`。它只做 GPUI 呈现层,后端全部经 `service.rs` 调用 caliberate 的 `caliberate-db` / `caliberate-library`。根 `Cargo.toml` 用两个 git 依赖锁定上游:zed 的 GPUI 全家桶(rev `92447a17`)与 caliberate(rev `f9c41d7`)。不新建任何数据库、不重写搜索、不实现导入器。

**Tech Stack:** Rust 1.98.1 / edition 2024、GPUI(git)、caliberate core/db/library(git)、rusqlite(间接)、serde_json、`ui::Table`、GPUI `uniform_list`。

**Spec:** `docs/superpowers/specs/2026-10-21-calibre2rust-design.md`(下称"spec";§编号均指该文档)

---

## Global Constraints

以下约束适用于**每一个** task,不再逐条重复:

- **工具链**:`rust-toolchain.toml` 固定 `channel = "1.98.1"`。安装走镜像:`RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1`。
- **构建命令**:`cargo build --target aarch64-apple-darwin`。**禁止** `cargo metadata`(本机实测 >15 分钟未完成,会解析所有 target 的依赖)。拉依赖用 `cargo fetch --target aarch64-apple-darwin`。
- **Cargo patch**:根 `Cargo.toml` 必须含
  `[patch.crates-io] async-task = { git = "https://github.com/zed-industries/async-task.git", rev = "015845a19fc31752c02367567d3b8900a406f9da" }`。
  缺了它,`gpui` 编译失败(`async_task::Task<T, M>` 类型不匹配)。
- **GPUI 特性**:`gpui_platform` 必须 `default-features = false, features = ["font-kit", "wayland", "x11", "runtime_shaders"]`。
  `runtime_shaders` 不可省 —— 本机无 Metal Toolchain,`xcrun metal` 不可用,缺它 `gpui_apple/build.rs:161-181` 会 `process::exit(1)`。
  `font-kit` 不可省 —— macOS 上它 gate 整个 Apple 文本系统。
- **`.cargo/config.toml`** 必须含 `[env] MACOSX_DEPLOYMENT_TARGET = "10.15.7"`。
- **Rust 规范**(zed `AGENTS.md` + caliberate `AGENTS.md`):
  - 禁止 `unwrap()`(用 `?` 传播);禁止 `let _ =` 丢弃可失败操作;禁止"总结代码"的注释。
  - 变量用完整词,不用缩写。
  - **禁止 `mod.rs`**;crate 用 `[lib] path = "src/lib.rs"`。
  - 用 `tracing` 做运行时诊断,不要 `println!`。
  - `cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check` 必须通过。
- **后端边界**(spec §4.4):**只有 `service.rs` 允许 import `caliberate_*` crate**。其它模块一律不碰 `caliberate-db` / `caliberate-library` / `rusqlite`。这条由 Task 2 的一个架构测试强制。
- **不引入** zed 的 `editor` / `search` / `picker` / `fs` / `assets` / `ui_input` 六个 crate(依赖过重,spec §3.2)。
- **不引入** Python 运行时、WebView、ORM、egui/eframe。
- **不修改 `../caliberate` 仓库**。发现 service 缺口,记入 `docs/caliberate-gaps.md`,不在前端绕过。
- **提交粒度**:每个 task 结束提交一次,commit message 用中文。
- **网络**:crates.io 索引直连极慢(实测 10 分钟卡在 "Updating crates.io index")。**必须**走镜像,否则每次构建都要等十几分钟:
  ```bash
  cargo build --target aarch64-apple-darwin \
    --config 'source.crates-io.replace-with="rsproxy"' \
    --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'
  ```
  建议把镜像写进 `~/.cargo/config.toml`(全局),**不要**写进本仓库的 `.cargo/config.toml`(保持仓库可移植)。同理 `rustup` 走 `RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static`。
  下文所有 `cargo build` / `cargo test` 命令默认已带该配置;若你把镜像写进了全局配置,可省略 `--config` 参数。

---

## Review Focus

spec 隐含但没有 task 的测试能覆盖的失败模式 —— 每条在下方对应 task 补了测试:

1. **书库为空 / 书库文件损坏 / 书库被另一个进程占用**。
   合理预期:窗口照常打开,显示可读的错误状态(不是白屏、不是 panic 堆栈)—— 对应 **Task 2**。
2. **书名/作者含非 ASCII、emoji、极长字符串**(calibre 用户常见)。
   合理预期:表格正确渲染不截断不 panic;排序稳定 —— 对应 **Task 4**。
3. **面板尺寸被持久化后,窗口变小导致书列表被挤没**。
   合理预期:恢复时把面板尺寸 clamp 到当前窗口,书列表始终有最小可用宽度 —— 对应 **Task 5**。
4. **超大数据集(10 万本)时滚动与排序**。
   合理预期:滚动流畅(虚拟列表),排序不阻塞 UI 线程 —— 对应 **Task 4**。
5. **用户拖动列宽到 0 / 负值 / 极大值**。
   合理预期:列宽被 clamp 到合理区间,不会导致表格渲染异常 —— 对应 **Task 5**。

---

## 文件结构

```
calibre2Rust/
├── Cargo.toml                      # workspace 根:zed git 依赖 + caliberate git 依赖 + async-task patch
├── rust-toolchain.toml             # channel = "1.98.1"
├── .cargo/config.toml              # MACOSX_DEPLOYMENT_TARGET
├── LICENSE                         # GPL-3.0-or-later
├── README.md                       # 用户已建(标题 calibre4rust),只追加不改标题
├── assets/
│   ├── NOTICE
│   └── icons/*.svg                 # 从 ../calibre/imgsrc/ 复制
├── crates/
│   └── calibre2rust-gui/
│       ├── Cargo.toml              # [lib] path = "src/lib.rs" + [[bin]] calibre2rust
│       ├── build.rs                # 扫描 assets/icons 生成静态表
│       └── src/
│           ├── lib.rs              # 库根:pub mod 声明
│           ├── main.rs             # 二进制入口
│           ├── app.rs              # 启动 + 全局状态 AppState
│           ├── service.rs          # 唯一接触 caliberate 的地方
│           ├── theme_provider.rs   # 自实现 ThemeSettingsProvider
│           ├── assets.rs           # 自实现 AssetSource
│           ├── actions.rs          # gpui actions! + 快捷键绑定
│           ├── window.rs           # 主窗口(工具栏/菜单栏/搜索栏/5面板/分隔条/状态栏)
│           ├── book_list.rs        # 书列表(表格视图)
│           └── view_state.rs       # 面板尺寸/列宽/列可见性/视图模式的持久化
├── docs/
│   ├── calibre-gaps.md             # caliberate service 缺口记录
│   └── superpowers/{specs,plans}/
└── tools/
    └── (后续阶段)
```

**为什么是这些文件**:
- `service.rs` 是唯一的后端出口 —— 其它 UI 文件因此可以完全不知道 SQLite 的存在(spec §4.4)。
- `window.rs` 只管骨架与面板布局,不管数据。
- `book_list.rs` 只管表格与行渲染。
- `view_state.rs` 独立出来,因为持久化 + clamp 逻辑需要单测,而它被 `window.rs` 和 `book_list.rs` 共用。

---

## Task 1: Workspace 骨架与 GPUI 启动

**Files:**
- Create: `Cargo.toml`、`rust-toolchain.toml`、`.cargo/config.toml`、`LICENSE`
- Create: `crates/calibre2rust-gui/Cargo.toml`、`crates/calibre2rust-gui/src/lib.rs`、`crates/calibre2rust-gui/src/main.rs`、`crates/calibre2rust-gui/src/theme_provider.rs`、`crates/calibre2rust-gui/src/assets.rs`、`crates/calibre2rust-gui/build.rs`
- Create: `assets/NOTICE`、`assets/icons/*.svg`
- Modify: `README.md`(只追加内容,**不改用户写的标题行**)
- Test: `crates/calibre2rust-gui/src/assets.rs`(内联 `#[cfg(test)]`)

**Interfaces:**
- Consumes: 无
- Produces:
  - crate `calibre2rust_gui`(lib)+ 二进制 `calibre2rust`
  - `calibre2rust_gui::assets::CalibreAssets` —— `pub struct CalibreAssets;`,实现 `gpui::AssetSource`
  - `calibre2rust_gui::theme_provider::CalibreThemeSettings` —— `pub struct CalibreThemeSettings;`,实现 `theme::ThemeSettingsProvider`
  - `calibre2rust_gui::app::run() -> anyhow::Result<()>` —— 二进制入口调用它
  - `calibre2rust_gui::assets::icon_bytes(name: &str) -> Option<&'static [u8]>` —— 按图标名取 SVG 字节

- [ ] **Step 1: 装工具链**

```bash
RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1
rustup run 1.98.1 rustc --version
```
Expected: `rustc 1.98.1 (...)`。若直连也通,可不用镜像。

- [ ] **Step 2: 写 workspace 根文件**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/calibre2rust-gui"]

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

caliberate-core       = { git = "https://github.com/sguzman/caliberate", rev = "f9c41d733e5bb26c913fb571cbd0e1f3b4b403eb", path = "crates/core" }
caliberate-db         = { git = "https://github.com/sguzman/caliberate", rev = "f9c41d733e5bb26c913fb571cbd0e1f3b4b403eb", path = "crates/db" }
caliberate-library    = { git = "https://github.com/sguzman/caliberate", rev = "f9c41d733e5bb26c913fb571cbd0e1f3b4b403eb", path = "crates/library" }

anyhow    = "1"
serde     = { version = "1", features = ["derive"] }
serde_json = "1"
tracing   = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid      = { version = "1", features = ["v4"] }
image     = { version = "0.25", default-features = false }

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

- [ ] **Step 3: 写 crate 清单与骨架**

`crates/calibre2rust-gui/Cargo.toml`:
```toml
[package]
name = "calibre2rust_gui"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
name = "calibre2rust_gui"
path = "src/lib.rs"

[[bin]]
name = "calibre2rust"
path = "src/main.rs"

[dependencies]
caliberate-core    = { workspace = true }
caliberate-db      = { workspace = true }
caliberate-library = { workspace = true }
gpui               = { workspace = true }
gpui_platform      = { workspace = true }
gpui_macros        = { workspace = true }
gpui_util          = { workspace = true }
ui                 = { workspace = true }
theme              = { workspace = true }
icons              = { workspace = true }
menu               = { workspace = true }
fuzzy              = { workspace = true }
component          = { workspace = true }
anyhow             = { workspace = true }
serde              = { workspace = true }
serde_json         = { workspace = true }
tracing            = { workspace = true }
tracing-subscriber = { workspace = true }
uuid               = { workspace = true }
image              = { workspace = true }

[build-dependencies]
```

`crates/calibre2rust-gui/src/lib.rs`:
```rust
pub mod app;
pub mod assets;
pub mod theme_provider;
```

`crates/calibre2rust-gui/src/main.rs`:
```rust
fn main() -> anyhow::Result<()> {
    calibre2rust_gui::app::run()
}
```

- [ ] **Step 4: 复制 calibre 图标并写 NOTICE**

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

- [ ] **Step 5: 写 `build.rs` 生成图标静态表**

`crates/calibre2rust-gui/build.rs`:
- 递归扫描 `../../assets/icons` 下的 `.svg`
- 生成到 `$OUT_DIR/icons_generated.rs`:
  ```rust
  pub const ICONS: &[(&str, &[u8])] = &[
      ("icons/add_book.svg", include_bytes!("../../assets/icons/add_book.svg")),
      // ... 每个图标一行
  ];
  ```
- 路径前缀固定 `icons/`(与 zed 的 `IconName::path()` 约定一致)
- `cargo:rerun-if-changed=../../assets/icons` 声明依赖

> `include_bytes!` 的路径相对于 `build.rs` 所在目录,故用 `../../assets/icons/...`;生成的文件里这个相对路径会被解析到 crate 根之外的 `assets/`,Cargo 允许。

- [ ] **Step 6: 写 `AssetSource` 实现与测试**

`crates/calibre2rust-gui/src/assets.rs`:
```rust
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

include!(concat!(env!("OUT_DIR"), "/icons_generated.rs"));

pub struct CalibreAssets;

pub fn icon_bytes(name: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .find(|(path, _)| *path == name)
        .map(|(_, bytes)| *bytes)
}

impl AssetSource for CalibreAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let prefix = format!("{path}/");
        Ok(ICONS
            .iter()
            .filter_map(|(icon_path, _)| icon_path.strip_prefix(&prefix))
            .map(SharedString::from)
            .collect())
    }
}
```

测试(内联 `#[cfg(test)]`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_known_calibre_icon() {
        let assets = CalibreAssets;
        let bytes = assets.load("icons/book.svg").expect("load should succeed");
        assert!(bytes.is_some(), "icons/book.svg 必须存在");
        assert!(bytes.expect("icon").starts_with(b"<svg"), "图标应为 SVG 内容");
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
        assert!(assets
            .load("icons/does_not_exist.svg")
            .expect("load should succeed")
            .is_none());
    }

    #[test]
    fn calibre_theme_variants_are_present() {
        // calibre 的深色主题变体必须一起复制过来
        assert!(icon_bytes("icons/close-for-dark-theme.svg").is_some());
    }
}
```

- [ ] **Step 7: 跑测试确认通过**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin assets::`
Expected: 4 passed

- [ ] **Step 8: 写 `ThemeSettingsProvider`**

`crates/calibre2rust-gui/src/theme_provider.rs`:
```rust
use gpui::{App, Font, FontFallbacks, Pixels};
use std::sync::OnceLock;
use theme::{ThemeSettingsProvider, UiDensity};

pub struct CalibreThemeSettings;

impl ThemeSettingsProvider for CalibreThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        static UI_FONT: OnceLock<Font> = OnceLock::new();
        UI_FONT.get_or_init(|| Font {
            family: ".SystemUIFont".into(),
            features: Default::default(),
            fallbacks: Some(FontFallbacks::from_fonts(vec![
                "PingFang SC".to_string(),
                "Hiragino Sans GB".to_string(),
                "STHeiti".to_string(),
            ])),
            weight: Default::default(),
            style: Default::default(),
        })
    }

    fn buffer_font<'a>(&'a self, cx: &'a App) -> &'a Font {
        self.ui_font(cx)
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

> `UiDensity` 的变体名以 `theme` crate 实际导出为准 —— 先 `grep -n "pub enum UiDensity" -A 8 ../zed/crates/theme/src/` 确认。若无 `Default` 实现,改为显式变体。

- [ ] **Step 9: 写启动代码与窗口**

`crates/calibre2rust-gui/src/app.rs`:
```rust
use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use theme::LoadThemes;

use crate::assets::CalibreAssets;
use crate::theme_provider::CalibreThemeSettings;

pub fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    gpui_platform::application()
        .with_assets(CalibreAssets)
        .run(|cx: &mut App| {
            theme::init(LoadThemes::JustBase, cx);
            theme::set_theme_settings_provider(Box::new(CalibreThemeSettings), cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(840.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| RootView),
            )
            .map_err(|error| anyhow::anyhow!("打开主窗口失败: {error}"))?;
            cx.activate(true);
        });
    Ok(())
}

struct RootView;

impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .p_4()
            .gap_2()
            .child("calibre2rust —— 中文字体测试:你好,世界")
            .child(ui::Button::new("点击我").on_click(|_, _window, _cx| {}))
    }
}
```

- [ ] **Step 10: 编译**

Run: `cargo build --target aarch64-apple-darwin`
Expected: 编译成功。
排查:
- `async_task::Task` 类型不匹配 → `[patch.crates-io]` 没生效。
- `cannot execute tool 'metal'` → `runtime_shaders` 没开。
- `caliberate-*` crate 找不到 → git 依赖没解析,检查 rev 与网络。

- [ ] **Step 11: 肉眼验证(需要录屏权限的机器)**

Run: `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected: 弹出窗口,中文"你好,世界"**不是豆腐块**,按钮可点。

- [ ] **Step 12: 追加 README(不改用户写的标题)**

在 `README.md` 现有内容之后追加:项目简介、当前进度、构建方法(含镜像与 `--target` 说明)、许可说明。
**保留用户写的 `# calibre4rust` 标题行原样。**

- [ ] **Step 13: 提交**

```bash
git add -A
git commit -m "feat: workspace 骨架与 GPUI 启动

- 根 Cargo.toml 引入 zed(gpui)与 caliberate 的 git 依赖,并重复 async-task patch
- 启用 runtime_shaders 绕开本机缺失的 Metal Toolchain
- 自实现 AssetSource(calibre 原版图标)与 ThemeSettingsProvider
- 中文走系统字体 fallback(PingFang SC / Hiragino Sans GB / STHeiti)"
```

---

## Task 2: service 层(caliberate 接入)

**Files:**
- Create: `crates/calibre2rust-gui/src/service.rs`
- Create: `docs/calibre-gaps.md`
- Modify: `crates/calibre2rust-gui/src/lib.rs`(加 `pub mod service;`)
- Modify: `crates/calibre2rust-gui/src/app.rs`(窗口显示书数)
- Test: `crates/calibre2rust-gui/src/service.rs`

**Interfaces:**
- Consumes: `caliberate_core::config::ControlPlane`、`caliberate_db::database::Database`、`caliberate_library::catalog::{LibraryCatalog, LibraryBook}`、`caliberate_library::query::{LibraryQuery, LibrarySortField, LibraryFacetKind, LibraryFacetValue}`、`caliberate_library::summary::LibrarySummaryPage`
- Produces:
  - `calibre2rust_gui::service::LibraryService` —— 结构体
  - `LibraryService::open(config: &ControlPlane) -> anyhow::Result<Self>`
  - `LibraryService::book_count(&self) -> anyhow::Result<usize>`
  - `LibraryService::query_summary_page(&self, query: &LibraryQuery) -> anyhow::Result<LibrarySummaryPage>`
  - `LibraryService::list_facets(&self, kind: LibraryFacetKind) -> anyhow::Result<Vec<LibraryFacetValue>>`
  - `LibraryService::get_book(&self, id: i64) -> anyhow::Result<Option<LibraryBook>>`
  - `calibre2rust_gui::service::default_library_query() -> LibraryQuery`
  - `calibre2rust_gui::service::SUMMARY_CHUNK_SIZE: usize` —— 2000

> **所有权注意**:`LibraryCatalog<'a>` 借用 `&'a dyn LibraryBackend`,不能自引用。本实现照搬 caliberate egui 侧的做法(`crates/gui/src/views.rs:90`):每次调用现建 `LibraryCatalog::new(&self.database)`。**不要**用 `Box::leak` 或自引用结构绕过。

- [ ] **Step 1: 确认 caliberate 在 macOS 上能编译**

**已知状态(2026-10-21 实测)**:在 `../caliberate` 下 `cargo check --workspace --all-targets` 于 macOS **零错误通过**(1m39s),全部 14 个 crate 均可编译。

本步是**复核**而非探索 —— 目的是确认在本计划的依赖组合下仍然成立(尤其 `caliberate-library` 会拖入 `caliberate-metadata` → `unrar` / `reqwest`)。

Run:
```bash
cd ../caliberate
cargo check --workspace --all-targets \
  --config 'source.crates-io.replace-with="rsproxy"' \
  --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'
```
Expected: `Finished dev profile ...`,零 `error`。

若将来 caliberate 升级后在本平台编译失败:
- 记录具体 crate 与错误到 `docs/caliberate-gaps.md` 的"平台缺口"小节。
- 只从本项目的依赖中移除**确实编译不过**的 crate。
- 若 `caliberate-library` 本身编译不过,退化为只依赖 `caliberate-db`,并在 gaps 里记录"分类浏览器与 calibre 库挂载不可用"。

- [ ] **Step 2: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_sorts_by_title_ascending() {
        let query = default_library_query();
        assert_eq!(query.sort, LibrarySortField::Title);
        assert!(!query.descending);
    }

    #[test]
    fn default_query_has_no_filters() {
        let query = default_library_query();
        assert!(query.title.is_none());
        assert!(query.author.is_none());
        assert!(query.metadata_filters.is_empty());
        assert!(query.limit.is_none());
    }

    #[test]
    fn opening_a_missing_library_directory_creates_it() {
        let dir = std::env::temp_dir().join(format!("calibre2rust-svc-{}", uuid::Uuid::new_v4()));
        let config = test_control_plane(&dir);
        let service = LibraryService::open(&config).expect("open should create the library");
        assert_eq!(service.book_count().expect("count"), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn opening_a_corrupt_database_is_an_error_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("calibre2rust-svc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        // 写一个不是 SQLite 的文件当数据库
        std::fs::write(dir.join("calibre.db"), b"this is not a sqlite database").expect("write");
        let config = test_control_plane(&dir);
        let result = std::panic::catch_unwind(|| LibraryService::open(&config));
        assert!(result.is_ok(), "打开损坏库不应 panic");
        assert!(result.expect("no panic").is_err(), "打开损坏库应返回 Err");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failing_database_opens_yields_a_readable_message() {
        // service.rs 的错误信息必须包含"书库"二字,便于 UI 直接展示给用户
        let dir = std::env::temp_dir().join(format!("calibre2rust-svc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("calibre.db"), b"not a database").expect("write");
        let config = test_control_plane(&dir);
        let message = match LibraryService::open(&config) {
            Ok(_) => panic!("应当失败"),
            Err(error) => format!("{error:#}"),
        };
        assert!(message.contains("书库"), "错误信息应提到书库: {message}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

`test_control_plane(&dir) -> ControlPlane` 是测试辅助函数:构造一个 `ControlPlane` 默认值,把 `db.path` 指向 `dir/calibre.db`、`paths` 指向 `dir`。
> `ControlPlane` 的字段以 `crates/core/src/config.rs:10` 为准 —— 实现时先读该文件确认可变性(是否 `Default` 派生、字段是否 `pub`)。

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin service::`
Expected: 编译错误 —— `LibraryService` 未定义。

- [ ] **Step 4: 实现 `service.rs`**

```rust
use anyhow::Context;
use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
use caliberate_library::catalog::{LibraryBook, LibraryCatalog};
use caliberate_library::query::{LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibrarySortField};
use caliberate_library::summary::LibrarySummaryPage;

pub const SUMMARY_CHUNK_SIZE: usize = 2000;

pub fn default_library_query() -> LibraryQuery {
    LibraryQuery {
        sort: LibrarySortField::Title,
        descending: false,
        ..Default::default()
    }
}

pub struct LibraryService {
    database: Database,
}

impl LibraryService {
    pub fn open(config: &ControlPlane) -> anyhow::Result<Self> {
        let database = Database::open_with_fts(&config.db, &config.fts)
            .context("打开书库数据库失败")?;
        Ok(Self { database })
    }

    fn catalog(&self) -> LibraryCatalog<'_> {
        LibraryCatalog::new(&self.database)
    }

    pub fn book_count(&self) -> anyhow::Result<usize> {
        Ok(self.catalog().list_books()?.len())
    }

    pub fn query_summary_page(&self, query: &LibraryQuery) -> anyhow::Result<LibrarySummaryPage> {
        Ok(self.catalog().query_summary_page(query)?)
    }

    pub fn list_facets(&self, kind: LibraryFacetKind) -> anyhow::Result<Vec<LibraryFacetValue>> {
        Ok(self.catalog().list_facets(kind)?)
    }

    pub fn get_book(&self, id: i64) -> anyhow::Result<Option<LibraryBook>> {
        Ok(self.catalog().get_book(id)?)
    }
}
```

> `LibraryQuery` 是否实现 `Default` 需确认;若没有,逐字段构造。

- [ ] **Step 5: 写架构边界测试**

在 `crates/calibre2rust-gui/src/lib.rs` 加测试,用源码扫描强制"只有 `service.rs` 能 import `caliberate_*`":

```rust
#[cfg(test)]
mod architecture_tests {
    use std::path::Path;

    #[test]
    fn only_service_module_imports_caliberate_crates() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in walk_src(&src) {
            let contents = std::fs::read_to_string(&entry).expect("read source");
            if contents.contains("use caliberate_") || contents.contains("caliberate_db::")
                || contents.contains("caliberate_library::")
            {
                let name = entry.file_name().expect("file name");
                if name != "service.rs" {
                    offenders.push(name.to_string_lossy().to_string());
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "只有 service.rs 可以依赖 caliberate crate,违反者: {offenders:?}"
        );
    }
}
```

`walk_src` 用 `std::fs::read_dir` 递归实现(不引第三方依赖)。

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin`
Expected: 全部通过(含架构边界测试)。

- [ ] **Step 7: 窗口显示书数**

在 `app.rs` 的 `RootView` 里加一个 `Entity<LibraryService>` 字段(通过 `cx.new` 构造),渲染时显示 `service.book_count()`。
**注意**:`book_count()` 每次调用都会全表 `list_books()` —— 大库上很贵。改为在 `LibraryService` 里缓存计数,并提供 `refresh_count()`;或在 Task 4 改用 `query_summary_page` 的 `total`。本 task 先用缓存字段 `book_count: usize`,在 `open()` 时算一次。

- [ ] **Step 8: 写 `docs/caliberate-gaps.md` 骨架**

记录已发现的 caliberate service 缺口。初始内容:
```markdown
# caliberate service 缺口

本文件记录 calibre2rust(GPUI 前端)在对接 caliberate 时发现的 service 能力缺口。

**规则**:前端不得绕过 service 直接查 SQLite 来填补这些缺口;应在此登记,由 caliberate 的架构师决定是否补齐。

| 缺口 | 影响 | 前端临时做法 | 状态 |
|---|---|---|---|
| 无 calibre 完整搜索语法 | 无法提供 calibre 等价搜索 | 只做自由文本 + 字段定向 | 待登记 |
| 无 `LibraryFacetKind::Formats` / `Identifiers` | 分类浏览器少 2 类 | 不显示 | 待登记 |
| `LibraryQuery` 单级排序 | 无多级排序 | 主字段 + id tie-breaker | 待登记 |
| `LibraryBackend` 只读 | 元数据编辑需绕到 `Database` | `service.rs` 直接调 `update_book_*` | 待登记 |
| macOS 编译状态 | 决定可依赖的 crate 集合 | 见下 | 验证中 |
```

- [ ] **Step 9: 提交**

```bash
git add -A
git commit -m "feat: service 层接入 caliberate,含架构边界测试

- LibraryService 封装 Database + LibraryCatalog
- 架构测试强制只有 service.rs 可以依赖 caliberate crate
- 记录 caliberate service 缺口到 docs/caliberate-gaps.md"
```

---

## Task 3: 主窗口骨架

**Files:**
- Create: `crates/calibre2rust-gui/src/actions.rs`、`crates/calibre2rust-gui/src/window.rs`
- Modify: `crates/calibre2rust-gui/src/lib.rs`、`crates/calibre2rust-gui/src/app.rs`
- Test: `crates/calibre2rust-gui/src/window.rs`

**Interfaces:**
- Consumes: `calibre2rust_gui::service::LibraryService`
- Produces:
  - `calibre2rust_gui::window::MainWindow` —— `Entity<MainWindow>`
  - `MainWindow::new(service: Entity<LibraryService>, cx: &mut App) -> Entity<MainWindow>`
  - `calibre2rust_gui::window::toolbar_actions() -> Vec<&'static str>`
  - `calibre2rust_gui::window::menubar_actions() -> Vec<&'static str>`
  - `calibre2rust_gui::window::PanelLayout` —— 枚举 `Wide` / `Narrow`
  - `calibre2rust_gui::window::PANEL_HIDE_THRESHOLD: f32` —— 10.0
  - `calibre2rust_gui::window::PANEL_SHOW_THRESHOLD: f32` —— 50.0
  - `calibre2rust_gui::actions::*` —— `gpui::actions!` 定义的 action 单元结构

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_toolbar_matches_upstream_calibre_default() {
        let actions = toolbar_actions();
        assert_eq!(actions[0], "Add Books");
        assert_eq!(actions[1], "Edit Metadata");
        assert!(!actions.contains(&"Help"), "macOS 工具栏不应有 Help");
        assert!(!actions.contains(&"Preferences"), "macOS 工具栏不应有 Preferences");
        // calibre gui2/__init__.py:359-376
        assert_eq!(
            actions,
            vec![
                "Add Books", "Edit Metadata", "---", "Convert Books", "View", "---",
                "Choose Library", "Donate", "---", "Fetch News", "Store", "Save To Disk",
                "Connect Share", "---", "Remove Books", "Tweak ePub",
            ]
        );
    }

    #[test]
    fn menubar_contains_preferences_and_help() {
        let actions = menubar_actions();
        // calibre gui2/__init__.py:331-340
        assert_eq!(
            actions,
            vec![
                "Add Books", "Edit Metadata", "Convert Books", "Choose Library",
                "Save To Disk", "Preferences", "Help",
            ]
        );
    }

    #[test]
    fn panel_thresholds_match_upstream_calibre() {
        assert_eq!(PANEL_HIDE_THRESHOLD, 10.0);
        assert_eq!(PANEL_SHOW_THRESHOLD, 50.0);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin window::`
Expected: 编译错误。

- [ ] **Step 3: 实现 `actions.rs`**

```rust
use gpui::actions;

actions!(
    calibre2rust,
    [
        AddBooks,
        EditMetadata,
        ConvertBooks,
        ChooseLibrary,
        SaveToDisk,
        OpenPreferences,
        ShowHelp,
        RemoveBooks,
        TweakEpub,
        ToggleLayout,
        ToggleCoverGrid,
        ToggleBookDetails,
        ToggleTagBrowser,
    ]
);
```

- [ ] **Step 4: 实现 `window.rs` 的骨架**

`MainWindow`:
```rust
pub struct MainWindow {
    service: Entity<LibraryService>,
    layout: PanelLayout,
    tag_browser_visible: bool,
    book_details_visible: bool,
    cover_browser_visible: bool,
    quick_view_visible: bool,
    book_list_visible: bool,
    left_panel_width: f32,
    right_panel_width: f32,
    status_message: String,
}
```

渲染结构:
1. 工具栏行:按 `toolbar_actions()` 逐项渲染;`"---"` 渲染成竖分隔线;其余渲染成 `ui::Button`,点击时发对应 action(本阶段多数动作是占位,只发 action 不实现)。
2. 搜索栏行:占位 —— 虚拟库按钮、清除 VL、排序、分组、搜索框占位、搜索按钮。搜索框本体在后续阶段(Task 7)实现,本阶段用 `ui::Button` 占位。
3. 中央容器:`flex_row` + 两侧 `flex_col`:
   - 左:`tag_browser` 占位面板
   - 中:`book_list` 占位面板(Task 4 填充)
   - 右:`book_details` 占位面板
   - 上:`cover_browser` 占位面板(默认隐藏)
   - 下:`quick_view` 占位面板(默认隐藏)
   - 4 条分隔条:用 `div().w(px(4)).on_drag(...)` 实现;拖动时更新对应面板宽度;宽度 < `PANEL_HIDE_THRESHOLD` 隐藏面板,> `PANEL_SHOW_THRESHOLD` 恢复。
4. 状态栏:显示 `status_message`(初始为空,后续由书列表填充书数)。

原生菜单:`cx.set_menus([...])`,内容来自 `menubar_actions()`。
> macOS 原生菜单用 `gpui::Menu` / `gpui::MenuItem`,见 `zed/crates/gpui/examples/set_menus.rs`。

快捷键:
- `Alt+Shift+L` → `ToggleLayout`(对应 calibre `gui2/central.py:437-443`)
- `Alt+Shift+G` → `ToggleCoverGrid`
- `Cmd+,` → `OpenPreferences`

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin window::`
Expected: 3 passed

- [ ] **Step 6: 编译并肉眼验证**

Run: `cargo build --target aarch64-apple-darwin` && `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected: 窗口显示工具栏(无 Help/Preferences)+ 搜索栏占位 + 三栏空面板 + 状态栏;拖动中缝可调整左栏宽度;拖过阈值面板隐藏。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "feat(app): 主窗口骨架,工具栏/菜单栏/搜索栏/五面板/分隔条"
```

---

## Task 4: 书列表(表格视图)

**Files:**
- Create: `crates/calibre2rust-gui/src/book_list.rs`
- Modify: `crates/calibre2rust-gui/src/window.rs`、`crates/calibre2rust-gui/src/lib.rs`
- Test: `crates/calibre2rust-gui/src/book_list.rs`

**Interfaces:**
- Consumes: `calibre2rust_gui::service::{LibraryService, default_library_query, SUMMARY_CHUNK_SIZE}`
- Produces:
  - `calibre2rust_gui::book_list::BookList` —— `Entity<BookList>`
  - `BookList::new(service: Entity<LibraryService>, cx: &mut App) -> Entity<BookList>`
  - `BookList::refresh(&mut self, cx: &mut Context<Self>)`
  - `BookList::row_count(&self) -> usize`
  - `BookList::selected_book_ids(&self) -> Vec<i64>`
  - `calibre2rust_gui::book_list::BookColumn` —— 枚举
  - `calibre2rust_gui::book_list::default_visible_columns() -> Vec<BookColumn>`
  - `calibre2rust_gui::book_list::column_label(column: BookColumn) -> &'static str`
  - `calibre2rust_gui::book_list::sort_field_for(column: BookColumn) -> Option<LibrarySortField>`
  - `calibre2rust_gui::book_list::format_row_cell(column: BookColumn, summary: &LibraryBookSummary) -> String`
  - `calibre2rust_gui::book_list::chunk_offsets(total: usize, chunk_size: usize) -> Vec<usize>`
  - `calibre2rust_gui::book_list::load_all_summaries(service: &LibraryService, query: &LibraryQuery) -> anyhow::Result<Vec<LibraryBookSummary>>`

```rust
pub enum BookColumn {
    Title,
    Authors,
    Series,
    Tags,
    Publisher,
    Rating,
    DateAdded,
    Format,
    Size,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_columns_hide_upstream_hidden_fields() {
        let columns = default_visible_columns();
        let names: Vec<&str> = columns.iter().map(|c| column_label(*c)).collect();
        // calibre gui2/library/views.py:1106 默认隐藏 last_modified/languages/formats/id/path/pages
        for hidden in ["last_modified", "languages", "formats", "id", "path", "pages"] {
            assert!(!names.contains(&hidden), "不应显示 {hidden}");
        }
        assert!(names.contains(&"title"));
    }

    #[test]
    fn title_column_maps_to_title_sort_field() {
        assert_eq!(sort_field_for(BookColumn::Title), Some(LibrarySortField::Title));
        assert_eq!(sort_field_for(BookColumn::Authors), Some(LibrarySortField::Authors));
        assert_eq!(sort_field_for(BookColumn::DateAdded), Some(LibrarySortField::DateAdded));
    }

    #[test]
    fn column_labels_are_human_readable() {
        assert_eq!(column_label(BookColumn::Title), "title");
        assert_eq!(column_label(BookColumn::DateAdded), "date_added");
    }

    #[test]
    fn row_cells_render_cjk_emoji_and_long_titles() {
        // 非 ASCII / emoji / 超长标题必须原样渲染,不 panic、不截断成乱码
        let summary = sample_summary("《三体》🚀 —— 一个非常非常长的书名用来测试超长情况");
        let cell = format_row_cell(BookColumn::Title, &summary);
        assert_eq!(cell, "《三体》🚀 —— 一个非常非常长的书名用来测试超长情况");
    }

    #[test]
    fn author_cell_joins_with_comma() {
        let summary = sample_summary_with_authors(vec!["刘慈欣".to_string(), "王晋康".to_string()]);
        assert_eq!(format_row_cell(BookColumn::Authors, &summary), "刘慈欣, 王晋康");
    }

    #[test]
    fn rating_cell_divides_by_two() {
        // calibre 内部评分 0–10 = 2×星
        let mut summary = sample_summary("X");
        summary.rating = Some(8);
        assert_eq!(format_row_cell(BookColumn::Rating, &summary), "4");
    }

    #[test]
    fn chunk_offsets_cover_the_whole_result_set() {
        // 10 万本、分块 2000 → 50 次取数,偏移量必须连续覆盖
        let offsets = chunk_offsets(100_000, 2_000);
        assert_eq!(offsets.len(), 50);
        assert_eq!(offsets[0], 0);
        assert_eq!(*offsets.last().expect("non-empty"), 98_000);
        // 相邻偏移量差值等于块大小
        for pair in offsets.windows(2) {
            assert_eq!(pair[1] - pair[0], 2_000);
        }
    }

    #[test]
    fn chunk_offsets_handle_small_and_empty_result_sets() {
        assert_eq!(chunk_offsets(0, 2_000), Vec::<usize>::new());
        assert_eq!(chunk_offsets(10, 2_000), vec![0]);
        assert_eq!(chunk_offsets(2_000, 2_000), vec![0]);
        assert_eq!(chunk_offsets(2_001, 2_000), vec![0, 2_000]);
    }
}
```

测试辅助 `sample_summary(title)` / `sample_summary_with_authors(authors)`:构造一个 `LibraryBookSummary` 字面量(字段以 `crates/library/src/summary.rs:8` 为准)。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin book_list::`
Expected: 编译错误。

- [ ] **Step 3: 实现列定义与排序映射**

```rust
pub fn default_visible_columns() -> Vec<BookColumn> {
    vec![
        BookColumn::Title,
        BookColumn::Authors,
        BookColumn::Series,
        BookColumn::Tags,
        BookColumn::Publisher,
        BookColumn::Rating,
        BookColumn::DateAdded,
        BookColumn::Format,
    ]
}

pub fn column_label(column: BookColumn) -> &'static str {
    match column {
        BookColumn::Title => "title",
        BookColumn::Authors => "authors",
        BookColumn::Series => "series",
        BookColumn::Tags => "tags",
        BookColumn::Publisher => "publisher",
        BookColumn::Rating => "rating",
        BookColumn::DateAdded => "date_added",
        BookColumn::Format => "format",
        BookColumn::Size => "size",
    }
}

pub fn sort_field_for(column: BookColumn) -> Option<LibrarySortField> {
    match column {
        BookColumn::Title => Some(LibrarySortField::Title),
        BookColumn::Authors => Some(LibrarySortField::Authors),
        BookColumn::Series => Some(LibrarySortField::Series),
        BookColumn::Tags => Some(LibrarySortField::Tags),
        BookColumn::Publisher => Some(LibrarySortField::Publisher),
        BookColumn::Rating => Some(LibrarySortField::Rating),
        BookColumn::DateAdded => Some(LibrarySortField::DateAdded),
        BookColumn::Format => Some(LibrarySortField::Format),
        BookColumn::Size => None,
    }
}
```

- [ ] **Step 4: 实现分块加载与行单元格格式化**

```rust
pub fn chunk_offsets(total: usize, chunk_size: usize) -> Vec<usize> {
    if chunk_size == 0 || total == 0 {
        return Vec::new();
    }
    let mut offsets = Vec::new();
    let mut offset = 0usize;
    while offset < total {
        offsets.push(offset);
        offset += chunk_size;
    }
    offsets
}

pub fn format_row_cell(column: BookColumn, summary: &LibraryBookSummary) -> String {
    match column {
        BookColumn::Title => summary.title.clone(),
        BookColumn::Authors => summary.authors.join(", "),
        BookColumn::Series => match &summary.series {
            Some(series) => match series.index {
                Some(index) => format!("{} [{}]", series.name, index),
                None => series.name.clone(),
            },
            None => String::new(),
        },
        BookColumn::Tags => summary.tags.join(", "),
        BookColumn::Publisher => summary.publisher.clone().unwrap_or_default(),
        BookColumn::Rating => summary.rating.map(|rating| (rating / 2).to_string()).unwrap_or_default(),
        BookColumn::DateAdded => summary.date_added.clone().unwrap_or_default(),
        BookColumn::Format => summary
            .formats
            .iter()
            .map(|format| format.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        BookColumn::Size => String::new(),
    }
}

pub fn load_all_summaries(
    service: &LibraryService,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    let mut rows = Vec::new();
    for offset in chunk_offsets(query_total_hint, SUMMARY_CHUNK_SIZE) {
        let page = service.query_summary_page(
            &query.clone().with_limit(SUMMARY_CHUNK_SIZE).with_offset(offset),
        )?;
        let fetched = page.books.len();
        rows.extend(page.books);
        if fetched < SUMMARY_CHUNK_SIZE {
            break;
        }
    }
    Ok(rows)
}
```
> `query_total_hint` 需要先知道总书数 —— 实现上先调一次 `query_summary_page(&query.with_limit(1))` 拿 `page.total`,再按 `chunk_offsets(total, SUMMARY_CHUNK_SIZE)` 循环。`LibraryBookSummary` 的 `series` 字段类型(`Option<LibrarySeriesSummary>`,含 `name` 与 `index: Option<f64>`)以 `crates/library/src/summary.rs:16` 为准。
> `LibraryQuery::with_limit` / `with_offset` 已在 caliberate 中提供(`crates/library/src/query.rs:111`, `:167`)。

- [ ] **Step 5: 实现 `BookList` 实体**

```rust
pub struct BookList {
    service: Entity<LibraryService>,
    rows: Vec<LibraryBookSummary>,
    columns: Vec<BookColumn>,
    sort_column: BookColumn,
    sort_descending: bool,
    selected: Vec<i64>,
    status_message: String,
}
```

- `refresh(&mut self, cx)`:用 `load_all_summaries` 重新加载;加载在后台线程做,完成后回到前台线程更新 `rows`。
  **注意**:caliberate 的 `Database` 是 `rusqlite::Connection`,非 `Send`。若 `LibraryService` 不能跨线程,加载只能在主线程做 —— 此时大库会卡 UI。**先确认 `Database` 是否 `Send`/`Sync`**;若不是,本 task 改为分帧加载(每帧加载一页),并把"Database 需支持后台查询"记入 `docs/caliberate-gaps.md`。
- 渲染:`ui::Table::new(columns.len())` + `.uniform_list(id, row_count, closure)`。
  行的渲染闭包里:`title` 用 `title`;`authors` 用 `authors.join(", ")`;`series` 用 `series` + `series_index`;`tags` 用 `tags.join(", ")`;`rating` 用 `rating.map(|r| r / 2)`(calibre 内部 0–10 = 2×星);`date_added` 用原值;`format` 用 `formats` 的名字逗号连接。
  非 ASCII / emoji / 超长文本:靠 `ui::Table` 的单元格裁剪,不做截断预处理。
- 多选:`on_click` 记录 `selected`;`Cmd` 点击追加,`Shift` 点击范围选择。
- 点击表头切换排序:`sort_field_for(column)` → 更新 `query.sort` 与 `query.descending` → 重新 `refresh`。
  **多级排序**:caliberate service 只支持单级(spec §9.2),本 task 用"主排序字段 + id 作为 tie-breaker"近似 —— 但 `LibraryQuery` 没有 tie-breaker 字段,因此**只按主字段排**;把"多级排序"记入 `docs/caliberate-gaps.md`。
- `status_message` 设为 `format!("{} 本书", rows.len())`,由 `window.rs` 读出显示在状态栏。

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin book_list::`
Expected: 3 passed

- [ ] **Step 7: 编译并肉眼验证**

Run: `cargo build --target aarch64-apple-darwin` && `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected:
- 状态栏显示书数;
- 点击表头"title"可切换升降序;
- 选中行有高亮;
- 载入一个真实书库(可用 `../caliberate` 的测试库或自己造几本)能正常显示。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "feat(app): 书列表表格视图,分块加载 + 单级排序 + 多选"
```

---

## Task 5: 视图状态持久化(面板尺寸/列宽/列可见性)

**Files:**
- Create: `crates/calibre2rust-gui/src/view_state.rs`
- Modify: `crates/calibre2rust-gui/src/window.rs`、`crates/calibre2rust-gui/src/book_list.rs`、`crates/calibre2rust-gui/src/lib.rs`
- Test: `crates/calibre2rust-gui/src/view_state.rs`

**Interfaces:**
- Consumes: `serde` / `serde_json`
- Produces:
  - `calibre2rust_gui::view_state::ViewState` —— 结构体(可序列化)
  - `ViewState::default() -> ViewState`
  - `ViewState::load(path: &Path) -> ViewState` —— 文件不存在或解析失败时返回 `default()`,**不 panic、不报错**
  - `ViewState::save(&self, path: &Path) -> anyhow::Result<()>`
  - `ViewState::clamp_to_window(&mut self, window_width: f32, window_height: f32)`
  - `calibre2rust_gui::view_state::MIN_BOOK_LIST_WIDTH: f32` —— 320.0
  - `calibre2rust_gui::view_state::MIN_COLUMN_WIDTH: f32` —— 24.0
  - `calibre2rust_gui::view_state::MAX_COLUMN_WIDTH: f32` —— 1200.0

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewState {
    pub left_panel_width: f32,
    pub right_panel_width: f32,
    pub tag_browser_visible: bool,
    pub book_details_visible: bool,
    pub cover_browser_visible: bool,
    pub quick_view_visible: bool,
    pub layout: String,              // "wide" | "narrow"
    pub view_mode: String,           // "table" | "grid"
    pub column_widths: BTreeMap<String, f32>,
    pub hidden_columns: Vec<String>,
    pub selected_sort_column: String,
    pub sort_descending: bool,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_state() {
        let state = ViewState::load(Path::new("/nonexistent/calibre2rust-view-state.json"));
        assert_eq!(state.left_panel_width, ViewState::default().left_panel_width);
    }

    #[test]
    fn malformed_file_yields_default_state() {
        let dir = std::env::temp_dir().join(format!("calibre2rust-vs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("state.json");
        std::fs::write(&path, "{ not json").expect("write");
        let state = ViewState::load(&path);
        assert_eq!(state, ViewState::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trips_through_json() {
        let dir = std::env::temp_dir().join(format!("calibre2rust-vs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("state.json");
        let mut state = ViewState::default();
        state.left_panel_width = 321.0;
        state.hidden_columns = vec!["publisher".to_string()];
        state.save(&path).expect("save");
        let loaded = ViewState::load(&path);
        assert_eq!(loaded.left_panel_width, 321.0);
        assert_eq!(loaded.hidden_columns, vec!["publisher".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clamping_keeps_the_book_list_usable() {
        let mut state = ViewState::default();
        state.left_panel_width = 2000.0;
        state.right_panel_width = 2000.0;
        state.clamp_to_window(1000.0, 800.0);
        assert!(state.left_panel_width + state.right_panel_width <= 1000.0 - MIN_BOOK_LIST_WIDTH);
        assert!(state.left_panel_width >= 0.0);
    }

    #[test]
    fn clamping_normalises_absurd_column_widths() {
        let mut state = ViewState::default();
        state.column_widths.insert("title".to_string(), -50.0);
        state.column_widths.insert("authors".to_string(), 99999.0);
        state.clamp_to_window(1000.0, 800.0);
        assert_eq!(state.column_widths.get("title"), Some(&MIN_COLUMN_WIDTH));
        assert_eq!(state.column_widths.get("authors"), Some(&MAX_COLUMN_WIDTH));
    }

    #[test]
    fn clamping_is_idempotent() {
        let mut state = ViewState::default();
        state.left_panel_width = 5000.0;
        state.clamp_to_window(1000.0, 800.0);
        let once = state.clone();
        state.clamp_to_window(1000.0, 800.0);
        assert_eq!(once, state);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin view_state::`
Expected: 编译错误。

- [ ] **Step 3: 实现 `view_state.rs`**

- `ViewState::default()`:左栏 280、右栏 360、`tag_browser_visible = true`、`book_details_visible = true`、`cover_browser_visible = false`、`quick_view_visible = false`、`layout = "wide"`、`view_mode = "table"`。
- `load(path)`:读文件失败 → `default()`;JSON 解析失败 → `default()`;解析成功但字段缺失 → serde 的 `#[serde(default)]` 补齐。
- `save(path)`:写 JSON(缩进 2 空格);父目录不存在时先 `create_dir_all`。
- `clamp_to_window(width, height)`:
  - 左右栏宽先各自 clamp 到 `[0, width - MIN_BOOK_LIST_WIDTH]`(若 `width < MIN_BOOK_LIST_WIDTH`,则各为 0)。
  - 再保证 `left + right <= width - MIN_BOOK_LIST_WIDTH`。
  - `column_widths` 每个值 clamp 到 `[MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH]`。
  - 宽度非有限(`NaN` / `inf`)时按 0 处理。
- **持久化位置**:用 caliberate 的 `ControlPlane.paths` 里的 GUI 状态目录;具体路径以 `crates/core/src/config.rs:1025` 的 `GuiConfig` 为准。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p calibre2rust_gui --target aarch64-apple-darwin view_state::`
Expected: 6 passed

- [ ] **Step 5: 接到窗口与书列表**

- `MainWindow` 持有 `ViewState`;窗口 resize 时调 `clamp_to_window`。
- 面板拖动结束时写回 `left_panel_width` / `right_panel_width`。
- `BookList` 读 `column_widths` / `hidden_columns` / `selected_sort_column` / `sort_descending` 初始化;变更时写回。
- 窗口关闭时 `save`。

- [ ] **Step 6: 编译并肉眼验证**

Run: `cargo build --target aarch64-apple-darwin` && `./target/aarch64-apple-darwin/debug/calibre2rust`
Expected: 拖动面板宽度 → 退出 → 重开,宽度保持;把窗口缩到很小再放大,书列表仍可见(未被面板挤没)。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "feat(app): 视图状态持久化,含窗口尺寸 clamp 与列宽归一化"
```

---

## Task 6: 收尾 —— 质量门禁与验收记录

**Files:**
- Create: `docs/superpowers/specs/2026-10-21-phase-abc-acceptance.md`
- Modify: `docs/caliberate-gaps.md`

**Interfaces:**
- Consumes: 全部前序 task
- Produces: 验收记录文档

- [ ] **Step 1: 全量质量门禁**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --target aarch64-apple-darwin
cargo build --target aarch64-apple-darwin
```
Expected: 全部通过,无 warning。
> 若 `clippy` 的 warning 存量来自上游依赖,不修(zed 的规则:不把既有 warning 存量变成无关范围),但**本 crate 自己的 warning 必须清零**。

- [ ] **Step 2: 写验收记录**

`docs/superpowers/specs/2026-10-21-phase-abc-acceptance.md` 记录:
- Phase A/B/C 验收清单逐条结果
- 实际验证命令与输出摘要
- **未验证项**(尤其是:像素渲染的肉眼确认需要录屏权限;caliberate 在 macOS 的编译覆盖范围)
- 与 spec 的偏离

- [ ] **Step 3: 更新 gaps 文档**

把 Phase A–C 期间发现的 caliberate service 缺口补进 `docs/caliberate-gaps.md`,每条写明:缺口、影响、前端临时做法、是否阻塞后续阶段。

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "docs: Phase A/B/C 验收记录与 caliberate 缺口清单"
```

---

## 附:本计划之后

- **Phase D/E(分类浏览器 + 搜索栏)**:单独一份计划。核心是 `LibraryFacetKind` → `LibraryQuery.metadata_filters` 的映射,以及自由文本/字段定向搜索。
- **Phase F/G(书籍详情 + 封面网格)**:单独一份计划。
- **Phase H(元数据编辑)**:先要确认 caliberate `update_book_*` 是否支持多值(标签/系列/语言),否则要等 service 扩展。
- **Phase I(工具栏动作与首选项)**:单独一份计划。
- **Phase J(阅读器)**:阻塞在 caliberate 侧 `crates/document` 与 `crates/formats` 就绪。

**每阶段一份独立计划** —— 不要把多个 Phase 塞进一份,计划会失控,且后续阶段的设计要建立在前一阶段的实测结果上。
