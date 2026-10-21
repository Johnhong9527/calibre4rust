# calibre2Rust 设计文档 —— caliberate 的 GPUI 前端

> 日期:2026-10-21(第 2 版,方向已修正)
> 状态:**待批准**
> 上游参考(行为基准):`../calibre`(calibre Python 源码树,`src/calibre/`,9.16.100 / commit `43b5c26`)
> 后端复用:`../caliberate`(`github.com/sguzman/caliberate`,commit `f9c41d7`)
> UI 框架来源:`../zed`(`github.com/zed-industries/zed`,`92447a17cc008225c73619f1c3f0cdefc8ababf3`)

> **第 1 版为什么被推翻**:第 1 版打算在 `calibre2Rust/` 里从零实现 db / 搜索语言 / 导入器 / 格式层(约 3 万行)。复核 `caliberate` 后发现该仓库已提供 47,687 行、14 个 crate 的成熟实现,其中 `crates/library/src/catalog.rs` 的 `LibraryCatalog` service、`crates/library/src/calibre/` 的 calibre 库只读兼容层、`crates/db` 的 SQLite 持久化**正是第 1 版要造的东西**。caliberate 的 `docs/project/priorities.md` 明确把"in-process Rust API for other Rust projects"列为 P0 adapter target —— 一个 GPUI 前端正落在它声明的架构位置上。重复造轮子既违背 caliberate 的架构边界,也没有工程收益。**本版改为:calibre2Rust 只写 GPUI 前端,后端全部复用 caliberate。**

---

## 1. 意图与动机

用 **GPUI**(Zed 的 UI 框架,以 git 依赖引入)实现一个 **calibre 主界面的桌面客户端**,后端数据与业务逻辑全部复用 caliberate 已有实现。

- calibre 的 GUI 交互模型(信息架构与操作流程)是行为基准,逐项对照 `../calibre/src/calibre/gui2/`。
- 凡是 caliberate 已有后端能力的功能(书库、搜索、分类、导入、转换、OPDS),GPUI 前端只做**呈现与交互**,不重复实现业务逻辑 —— 遵循 caliberate `ARCHITECTURE.md` 的 *"Protocol handlers must not independently reimplement database queries, search rules, or path-selection logic. They should call the same library service used elsewhere."*
- 目标平台 **macOS 优先**(GPUI 三平台都支持,但一期只保证 macOS)。

**为什么用 GPUI 而不是 caliberate 现有的 egui**:GPUI 是原生 macOS 渲染、与 Rust 后端同构、支持多窗口/拖拽/富文本/虚拟滚动,已在 Zed 生产环境验证;egui 的 `views.rs` 已有 13,628 行,是 caliberate 自己承认的 *"legacy concentration"*。用 GPUI 在**独立前端工程**里重做,不必在 caliberate 仓库内推倒 1.36 万行的既有实现。

---

## 2. 目标 / 非目标 / 反目标

### 2.1 目标

| # | 目标 | 验收标准 |
|---|---|---|
| G1 | GPUI 以 git 依赖引入并跑通 | `cargo build` 通过;窗口打开、中文正常显示 |
| G2 | 打通 caliberate service | 前端能打开一个 caliberate 书库并列出全部书 |
| G3 | 主窗口骨架 | 工具栏 / 原生菜单栏 / 搜索栏 / 5 面板 / 分隔条 / 状态栏,布局对齐 calibre |
| G4 | 书列表(表格视图) | 全列可显示、可排序(多级)、可调列宽、可隐藏、虚拟滚动、多选 |
| G5 | 搜索栏 | 全局搜索 + 分类过滤联动 + 结果计数,行为对齐 calibre |
| G6 | 分类浏览器(左栏) | 作者/标签/系列/出版商/评分/语言 六类 facet,带计数,点击过滤 |
| G7 | 书籍详情面板(右栏) | 封面 + 元数据表 + 格式列表 + 评论,值可点击 |
| G8 | 封面网格视图 | 与表格视图切换,共享同一数据源 |
| G9 | 元数据编辑 | 单本 + 批量,字段覆盖 calibre 基本字段 |
| G10 | 视图/布局状态持久化 | 面板尺寸、列宽、列可见性、视图模式持久化 |
| G11 | 工具栏动作 | 添加书籍 / 删除书籍 / 转换书籍 / 保存到磁盘 / 虚拟库 / 首选项 |
| G12 | 中英双语可切换 | 设置中切换,立即生效 |

### 2.2 非目标

- **不重写后端**。db、搜索、分类、导入、转换、OPDS、附件同步一律用 caliberate 的 crate。
- **不实现 calibre 的 Python 插件生态**。
- **不追求与 calibre 的 `metadata.db` 二进制兼容** —— 但**必须能挂载并读取**既有 calibre 书库(caliberate 的 `CalibreLibraryBackend` 已实现)。
- **不做阅读器**(P1,见 §6 Phase I)。egui 侧的阅读器也是占位(caliberate `docs/inventory/gui-parity-validation.md` 记录:*"Reader TOC is reconstructed from heading scans instead of source-native TOC structures"*),先做 GUI 主体。
- **不改 caliberate 仓库**。本项目是它的**消费方**。若发现 caliberate 有缺陷,记录到本仓库的 issue 清单,由 caliberate 的架构师决定是否修。
- 一期只保证 macOS。

### 2.3 反目标

- **把 GPUI 当装饰**。GPUI 只做呈现层;任何"图省事在 UI 里直接查 SQLite"的做法都违反 caliberate 的服务边界。
- **半成品交互**。空表格、点了没反应的面板、吞掉的错误,比没有更糟。
- **静默失败**。service 调用失败必须显示可读错误,并提供 `tracing` 事件。
- **一次性大爆炸**。不接受 8000 行的单文件 GUI 提交。

---

## 3. 约束

### 3.1 硬约束

| 约束 | 细节 |
|---|---|
| Rust 工具链 | zed 要求 `1.98.1`(`zed/rust-toolchain.toml`);caliberate 未固定工具链。本项目 `rust-toolchain.toml` pin `1.98.1`。安装需走镜像:`RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1` |
| **Metal Toolchain 缺失** | 本机 `xcrun metal --version` 报 *missing Metal Toolchain*(它是 Xcode 的独立可下载组件,不在 CLT 里)。`gpui_apple/build.rs:161-181` 会 `process::exit(1)` 直接构建失败。**解法:启用 `gpui_platform` 的 `runtime_shaders` 特性**,让 shader 运行时由 Metal framework 编译,完全跳过 `xcrun metal` |
| macOS 构建 | 需完整 Xcode(已安装)。`cmake` 本机缺失,当前不依赖 |
| `MACOSX_DEPLOYMENT_TARGET` | zed 设 `10.15.7`;`gpui_apple/build.rs:137` 的 `-mmacosx-version-min` 是传给 `xcrun metal` 的,与 rustc 侧独立,照抄 zed 的 `.cargo/config.toml` `[env]` 块即可 |
| Cargo patch 继承 | **`[patch.crates-io]` 只在消费方 workspace 根生效**。zed 用了 `async-task` 的 fork(API 泛型化为 `Task<T, M>` / `Runnable<M>`),**必须在本项目根 Cargo.toml 重复声明**该 patch,否则 `gpui` 编译失败 |
| caliberate 兼容性 | caliberate 根 `Cargo.toml` **无 `[patch.crates-io]`**,无 `workspace.package` 继承 → 作为 git 依赖消费时无需额外 patch。其 crate 也未设 `publish = false`,git 依赖直接可用 |
| caliberate 平台 | caliberate 官方保证 Windows/Linux,macOS 需自行验证(见 §7.3) |
| 网络 | crates.io 下载不稳定;`cargo metadata` 会解析所有 target 并拖入 windows/linux/wasm 依赖(本机实测 >15 分钟未完成)→ **用 `cargo build --target aarch64-apple-darwin` 与 `cargo fetch --target aarch64-apple-darwin`** |
| 许可 | caliberate 是 GPL-3.0-or-later;calibre 是 GPLv3;zed 的 `ui`/`theme`/`icons` 是 GPL-3.0-or-later。**整仓 GPL-3.0-or-later** |

### 3.2 GPUI 的能力边界(直接影响设计)

| 事实 | 影响 |
|---|---|
| crates.io 的 `gpui 0.2.2` 是**旧 API**(`Application::new()`、bundled blade-graphics) | **必须用 git + rev**,不能用 crates.io 版本 |
| 当前 zed main 的入口是 `gpui_platform::application()`(`gpui::Application::with_platform`) | 启动代码按此写 |
| `gpui_platform` 的 `default = ["multithreaded"]` 指向 wasm | 消费方 `default-features = false`;`font-kit` 在 macOS 上**必须开**(它 gate 整个 Apple 文本系统) |
| **`ui` 组件在没有 `ThemeSettingsProvider` 时 panic** | `theme::theme_settings(cx)` 在未注册 provider 时直接 panic(`crates/theme/src/theme_settings_provider.rs:41`),而 `ui::Table` 默认 `use_ui_font: true` 会调用它 → **必须自己实现该 trait(5 个方法:`ui_font` / `buffer_font` / `ui_font_size` / `buffer_font_size` / `ui_density`,约 40 行)**,以免引入 zed 的 `theme_settings`(它拖入 `settings` → `fs` → `notify`,并需再补两个 patch) |
| `theme` 需要显式初始化 | 任何 `ui` 组件渲染前必须 `theme::init(theme::LoadThemes::JustBase, cx)` |
| gpui **没有** tree 组件 | 分类浏览器树自实现;`ui::TreeViewItem` 可复用行渲染 |
| gpui **没有** input 组件 | 单行输入框参考 `zed/crates/gpui/examples/input.rs`(920 行)自实现。**明确不用** `ui_input::InputField`:它要求 `ERASED_EDITOR_FACTORY`,唯一 setter 在 `editor` crate,等于拖入 Zed 整个编辑器栈 |
| gpui **没有** table 组件 | 用 `ui::Table`(`zed/crates/ui/src/components/data_table.rs:356`),提供可调列宽、列筛选、斑马纹、`uniform_list` 虚拟滚动 |
| GPUI 原生支持 SVG | `img.rs:216` 把 `svg` 列入支持扩展名,用 `resvg`/`usvg` 渲染 → calibre 的图标**无需**上游的 `rsvg-convert` 栅格化步骤 |
| `icons` crate 只提供图标名枚举,不含图片 | `ui::Icon` 通过 `IconName::path()` → `"icons/<stem>.svg"` 向 `AssetSource` 取图 |
| `text` crate 是协作式 rope buffer,不是富文本 | 富文本用 `gpui::StyledText` + `TextRun` + `TextLayout` |
| `assets` crate 内嵌 Zed 全部字体与图标 | **不引入**(无中文字形,且体积大),自建 `AssetSource` |

### 3.3 依赖策略

- **GPUI 侧**(全部 git 依赖,固定 rev `92447a17`):`gpui`、`gpui_platform`、`gpui_macros`、`gpui_util`、`ui`、`theme`、`icons`、`menu`、`fuzzy`、`component`。
- **caliberate 侧**(git 依赖,固定 rev `f9c41d7`):`caliberate-core`、`caliberate-db`、`caliberate-library`、`caliberate-assets`、`caliberate-metadata`、`caliberate-conversion`。
  - **不引入** `caliberate-gui`(egui 前端)—— 那是本项目的平行实现,不是依赖。
  - **不引入** `caliberate-app` 的二进制部分;但**可以**用 `caliberate_app::bootstrap::init()` 复用配置加载与运行时目录初始化(它是 `pub` 的)。
- **不引入**:`editor`、`search`、`picker`、`fs`、`assets`、`ui_input`(zed 侧,依赖过重)。
- **不引入**:Python 运行时、WebView、ORM、egui/eframe。

---

## 4. 架构

### 4.1 定位:caliberate 的一个 GPUI 前端

```
┌──────────────────────────────────────────────────────────┐
│  calibre2Rust (本仓库)                                    │
│                                                          │
│   crates/calibre2rust-gui   ← 唯一新写的 crate            │
│     ├── GPUI 呈现层                                       │
│     ├── 交互状态(选中/排序/过滤/面板尺寸)                │
│     └── 只调用 service API,不含 SQL、不含业务规则         │
└──────────────┬───────────────────────────┬───────────────┘
               │ Rust API                  │ git 依赖
               ▼                           ▼
┌──────────────────────────┐   ┌──────────────────────────┐
│  caliberate (只读消费)     │   │  zed (只读消费)           │
│  core/db/library/assets   │   │  gpui/gpui_platform/ui    │
│  metadata/conversion      │   │  theme/icons/menu         │
│  → LibraryCatalog service │   │  → GPUI 渲染             │
└──────────────────────────┘   └──────────────────────────┘
```

**契约**:本项目是 caliberate 的**一个前端实现**,不是它的分支,也不是它的替代品。
- caliberate 的 `crates/gui`(egui)保持原样,继续是它仓库内的默认前端。
- 两者共享同一个 service API;**本项目不得绕过 `LibraryCatalog`/`LibraryBackend` 直接查 SQLite**。
- 若本项目需要 service 没有的能力(例如 calibre 完整搜索语法),优先在 caliberate 侧扩展 service,而不是在 GUI 侧模拟。

### 4.2 crate 划分

**只写一个 crate。** 理由:前端代码量预计 5,000–8,000 行(对比 caliberate 的 egui 前端 16,876 行),拆多 crate 只会增加 ceremony。

```
calibre2Rust/
├── Cargo.toml                      # workspace 根 + [patch.crates-io] async-task
├── rust-toolchain.toml             # channel = "1.98.1"
├── .cargo/config.toml              # MACOSX_DEPLOYMENT_TARGET
├── LICENSE                         # GPL-3.0-or-later
├── README.md
├── assets/
│   ├── NOTICE                      # 图标来源与许可
│   └── icons/*.svg                 # 从 ../calibre/imgsrc/ 复制
├── crates/
│   └── calibre2rust-gui/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs              # 库根
│           ├── main.rs             # 二进制 calibre2rust
│           ├── app.rs              # 启动 + 全局状态 AppState
│           ├── service.rs          # caliberate service 适配层(唯一接触后端的地方)
│           ├── theme_provider.rs   # 自实现 ThemeSettingsProvider
│           ├── assets.rs           # 自实现 AssetSource
│           ├── i18n.rs             # 中英文案
│           ├── actions.rs          # gpui actions! + 快捷键
│           ├── window.rs           # 主窗口(工具栏/菜单栏/搜索栏/5面板/分隔条/状态栏)
│           ├── tag_browser.rs      # 分类浏览器(左栏)
│           ├── book_list.rs        # 书列表(表格视图)
│           ├── cover_grid.rs       # 封面网格视图
│           ├── book_details.rs     # 书籍详情(右栏)
│           ├── search_box.rs       # 搜索栏
│           ├── input_field.rs      # 单行输入框
│           ├── edit_metadata.rs    # 元数据编辑对话框
│           ├── preferences.rs       # 首选项
│           └── views/              # (可选)视图状态持久化
├── docs/superpowers/specs/
└── docs/superpowers/plans/
```

### 4.3 依赖图

```
calibre2rust-gui
   ├── caliberate-core / db / library / assets / metadata / conversion   (git)
   ├── gpui / gpui_platform / gpui_macros / gpui_util                    (git)
   ├── ui / theme / icons / menu / fuzzy / component                     (git)
   └── serde / serde_json / anyhow / tracing / uuid / image
```

无环。`calibre2rust-gui` 是叶子节点,不依赖任何本地 crate。

### 4.4 service API 契约(前端唯一的后端入口)

**唯一允许接触后端的地方是 `service.rs`。** 其余 UI 模块只调用 `service.rs` 暴露的方法。

caliberate 已提供(签名取自 `crates/library/src/catalog.rs` 与 `crates/library/src/query.rs`):

```rust
// crates/library/src/catalog.rs
pub trait LibraryBackend {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>>;
    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>>;
    fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>>;
    fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>>;
    fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage>;
    fn query_summary_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage>;
    fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>>;
    fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>>;
    fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>>;
    fn resolve_content_format(&self, book_id: i64, format: &str) -> CoreResult<Option<LibraryContent>>;
}
pub struct LibraryCatalog<'a> { /* new(backend: &'a dyn LibraryBackend) */ }

// crates/library/src/query.rs
pub struct LibraryQuery {
    pub title: Option<String>, pub author: Option<String>, pub tag: Option<String>,
    pub series: Option<String>, pub publisher: Option<String>, pub language: Option<String>,
    pub identifier: Option<String>, pub format: Option<String>,
    pub limit: Option<usize>, pub offset: Option<usize>,
    pub sort: LibrarySortField, pub descending: bool,
    pub metadata_filters: Vec<LibraryMetadataFilter>,
}
pub enum LibraryFacetKind { Authors, Tags, Series, Publishers, Ratings, Languages }
pub struct LibraryFacetValue { pub id: i64, pub name: String, pub count: i64 }
pub struct LibrarySummaryPage { pub books: Vec<LibraryBookSummary>, pub total: usize, pub offset: usize, pub limit: Option<usize> }
```

**本项目新增的适配层**(`service.rs`):

```rust
pub struct LibraryService {
    catalog: LibraryCatalog<'static>,   // 见下方所有权说明
    // 实际实现用 enum/Box 持有 backend
}
```

> **所有权问题**:`LibraryCatalog<'a>` 借用 `&'a dyn LibraryBackend`,不能自引用。egui 侧的做法是每次需要时 `LibraryCatalog::new(&self.db)` 现建一个(`crates/gui/src/views.rs:90,8461,8499`)。GPUI 前端照此模式:`service.rs` 持有 `Database`(或 backend),每次调用现建 `LibraryCatalog`。**不要**试图用 `Box::leak` 或自引用结构绕过。

```rust
impl LibraryService {
    pub fn open(config: &ControlPlane) -> CoreResult<Self>;
    pub fn list_books(&self) -> CoreResult<Vec<LibraryBook>>;
    pub fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage>;
    pub fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>>;
    pub fn search(&self, query: &str) -> CoreResult<Vec<LibraryBook>>;
    pub fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>>;
    pub fn list_formats(&self, id: i64) -> CoreResult<Vec<LibraryFormat>>;
    pub fn resolve_content(&self, id: i64, format: &str) -> CoreResult<Option<LibraryContent>>;
    pub fn update_book(&self, id: i64, patch: &BookPatch) -> CoreResult<bool>;
}
```

`BookPatch` 是本项目定义的前端侧补丁结构(标题/作者/标签/系列/序号/评分/出版商/语言/评论/标识符),由 `service.rs` 翻译成 caliberate `Database::update_book_*` 调用。

**book list 读取路径**(照搬 egui 的分块模式,`crates/gui/src/views.rs:84-101`):用 `query_summary_page` + `limit`/`offset` 分块拉取,而不是一次性 `list_books`。大库必须这样。

---

## 5. 关键设计决策

### D1. GPUI 以 git 依赖引入

**决策**:根 `Cargo.toml` 用 `git = "https://github.com/zed-industries/zed"` + `rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3"` 引入 GPUI 全家桶。

**理由**:用户明确要求"直接引用 zed 仓库内 gpui crates 作为 git 依赖";且 crates.io 的 `gpui 0.2.2` 是旧 API,用了编译不过;固定 rev 解决 zed main 移动靶问题。

```toml
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

[patch.crates-io]
async-task = { git = "https://github.com/zed-industries/async-task.git", rev = "015845a19fc31752c02367567d3b8900a406f9da" }
```

`runtime_shaders` 不可省(见 §3.1)。`font-kit` 不可省(macOS 文本系统)。

**已实测**:2026-10-21 在临时目录用上述 manifest 完整编译并运行 GPUI 栈 —— 编译通过、进程存活、无 Metal/shader 错误。
**未实测**:像素渲染的肉眼确认 —— 本机 `screencapture`/`osascript` 被系统权限(录屏/辅助功能)拒绝。**Phase A 必须在有录屏权限的机器上补这一步**。

### D2. 依赖 caliberate 的方式

**决策**:git 依赖,`git = "https://github.com/sguzman/caliberate"`,`rev = "f9c41d733e5bb26c913fb571cbd0e1f3b4b403eb"`(本地 `../caliberate` 的 HEAD,与 `origin/main` 一致)。

**理由**:
1. 用户已拍板"复用 caliberate 后端"。
2. 固定 rev 使构建可复现,不依赖兄弟目录的状态;将来 `../caliberate` 前进不会突然改变本项目行为。
3. caliberate 无 `[patch.crates-io]`、无 `workspace.package` 继承 → 作为 git 依赖消费**不需要**额外 patch 或版本冲突处理。
4. caliberate 内部 crate 之间的 `path` 依赖在 git checkout 内自动解析(与 zed 同理)。

**代价**:caliberate 后端改动需先 push 才能被本项目看到。**开发期对策**:需要联调 caliberate 时,临时把 git 依赖换成 `path = "../caliberate/crates/db"` 之类;**提交前必须换回 git 依赖**,以免提交一个只能在本地编译的 manifest。

**验证状态**:待做(Phase A 第一项)。

### D3. 主界面结构

**决策**:严格对齐 calibre 5+ 的主窗口(calibre 5 起是 toolbar-driven,不再是 File/Edit 菜单结构)。

**顶层结构**(自上而下):
1. **工具栏**(macOS 默认布局,`calibre/src/calibre/gui2/__init__.py:359-376`):
   ```
   Add Books | Edit Metadata | ‖ Convert Books | View | ‖ Choose Library | Donate | ‖
   Fetch News | Store | Save To Disk | Connect Share | ‖ Remove Books | Tweak ePub
   ```
   注意:macOS 默认**没有** Help / Preferences(它们在原生菜单栏)。
2. **macOS 原生菜单栏**(`gui2/__init__.py:331-340`):
   `Add Books / Edit Metadata / Convert Books / Choose Library / Save To Disk / Preferences / Help`
3. **搜索栏行**:
   `[虚拟库按钮] [清除VL] [排序▾] [分组▾] [搜索框] [FTS图标·前置] [齿轮图标·前置] [解析错误图标·后置] [搜索] [高亮]`
4. **中央容器**:5 面板 + 4 分隔条(`gui2/central.py:394-397`)。
   - 宽布局:左=分类浏览器,中=书列表,右=书籍详情,上=封面浏览器,下=快速预览。
   - 窄布局(Alt+Shift+L 切换):左=分类浏览器(0.25 宽),右=封面浏览器(0.35 宽),上=快速预览(0.2 高),下=书籍详情(0.3 高),中=书列表。
   - 拖分隔条低于 10px 隐藏面板,超过 50px 恢复(`gui2/central.py:39-40`)。
   - 默认可见:分类浏览器 ✓、书籍详情 ✓、书列表 ✓;封面浏览器 ✗、快速预览 ✗。
5. **状态栏**:显示当前书数。

**GPUI 映射**:

| calibre 构件 | GPUI 实现 |
|---|---|
| 书列表表格 | `ui::Table`(`crates/ui/src/components/data_table.rs:356`) |
| 分类浏览器树 | 自实现:`uniform_list` + 缩进 + 展开态;行渲染可复用 `ui::TreeViewItem` |
| 输入框 | 参考 `crates/gpui/examples/input.rs` 自实现 `InputField` |
| 主题/字体 provider | 自实现 `ThemeSettingsProvider`(5 方法);启动时 `theme::init(LoadThemes::JustBase, cx)` |
| 详情面板 HTML(评论) | 自实现受限 HTML → `Vec<TextRun>` 渲染器(`b/i/span/a/h1-h6/br/ul/ol/li/img/table`);不引入 Zed 的 markdown crate |
| 封面 | `gpui::img()` + `ImageSource`,`image` crate 解码 |
| 菜单 / 右键菜单 | 原生 `cx.set_menus([...])`;弹出菜单用 `ui::ContextMenu` / `ui::Popover` |
| 模态对话框 | `ui::Modal`;系统确认用 `window.prompt(PromptLevel::...)` |
| 文件选择 | `cx.prompt_for_paths(PathPromptOptions { ... })` |
| 拖拽 | `.on_drag()` / `.on_drop::<T>()` |
| 快捷键 | `gpui::actions!` + `KeyBinding::new` + `cx.bind_keys` |

### D4. 分类浏览器用 `LibraryFacetKind`

calibre 的分类浏览器有 9 类(作者/语言/系列/格式/出版商/评分/新闻/标签/标识符),caliberate 的 `LibraryFacetKind` 提供 6 类(Authors/Tags/Series/Publishers/Ratings/Languages)。

**决策**:实现 caliberate 支持的 6 类;**格式(Formats)与标识符(Identifiers)不单独做 facet 类别** —— 格式是每本书的属性,calibre 把"格式"当分类是因为它把格式存成 link 表;caliberate 的 `LibraryQuery` 有 `with_format` 过滤器但没有 `LibraryFacetKind::Formats`。**这是 caliberate 的 service 缺口**,记入 §9 的 caliberate issue 清单,前端先不做。

分类项点击 → 追加到 `LibraryQuery.metadata_filters`(`LibraryMetadataFilter { field, mode: Include, value }`)。`mode: Exclude` 对应 calibre 分类浏览器的"排除"操作。

`LibraryFacetValue { id, name, count }` 直接给出计数,满足 calibre 的"显示条目数"。

### D5. 书列表用 `query_summary_page`

**决策**:书列表数据源 = `catalog.query_summary_page(&query)`,**不是** `list_books`。

理由:大库必须分页;`LibraryBookSummary` 恰好包含表格视图需要的全部字段(`id/title/format/formats/authors/tags/series/rating/publisher/languages/has_cover/date_added/date_modified/pubdate`)。
照搬 egui 的分块模式(`crates/gui/src/views.rs:84-101`):`with_limit(chunk_size).with_offset(offset)` 循环拉取,直到取满 `total`。

列定义对齐 calibre(`gui2/library/views.py:1106` 默认隐藏 `last_modified / languages / formats / id / path / pages`)。
排序:`LibrarySortField` 提供 Id/Title/Authors/Series/Tags/Format/Rating/Publisher/Languages/DateAdded/DateModified/PubDate + `descending`。**多级排序 caliberate 的 service 目前只有单级** —— 前端可先按"主排序字段 + 次级 tie-breaker(id)"实现,真正的多级排序记入 §9 缺口清单。

### D6. 书籍详情

- 内部是 splitter:左侧封面,右侧元数据表(calibre `gui2/book_details.py:1391` 的 `DetailsLayout`)。
- 元数据表按 `book_display_fields` 顺序渲染;值可点击(作者 → 加进搜索、标识符 → 复制)。
- 评论:caliberate 存的是 HTML 还是 markdown 需在实现时确认(`crates/db` 的 comments 字段);渲染走 D3 的受限 HTML 渲染器,markdown 用 `pulldown-cmark`(caliberate 已在用,见 `crates/gui/Cargo.toml`)。
- 格式列表:`catalog.list_formats(book_id)`。

### D7. 搜索

**决策**:第一版用 caliberate 的 `catalog.search_books(query)`(子串式),**不**实现 calibre 完整搜索语法。

calibre 的搜索语言很复杂(运算符 `= ~ ^ \`、层级 `=. =..`、三引号、日期精度、评分整除、三态布尔、`all:` / `search:` / `vl:` / `@UserCategory:` 位置 —— 完整语义见第 1 版设计文档与 `calibre/src/calibre/db/search.py`)。caliberate 的 service 目前只有 `search_books(&str)`,**没有** calibre 语法解析器。

**这是最大的 service 缺口**,记入 §9。前端第一版提供:
- 自由文本搜索(全字段)
- 字段定向:`title:` / `author:` / `tag:` / `series:` / `publisher:` / `format:` → 映射到 `LibraryQuery` 的对应字段
- 分类过滤:facet 点击 → `metadata_filters`
- 组合:字段条件 AND,分类过滤 AND

**不做**:正则、层级、日期比较、评分比较、保存搜索(虚拟库在 Phase G)。

### D8. 元数据编辑

`service.rs` 的 `update_book(id, &BookPatch)` 翻译成 caliberate `Database::update_book_*` 调用(`crates/db/src/database.rs:2602+`:`update_book_title` / `update_book_sort` / `update_book_author_sort` / `update_book_pubdate` / `update_book_tags`? / `update_book_rating`? 等 —— 实现时以实际存在的为准,缺的记入 §9)。

编辑对话框字段:标题、作者、系列 + 序号、标签、出版商、评分(0–10)、语言、评论、标识符。
**标签/系列/语言是多值**,caliberate 的 `update_book_*` 是否支持多值需在实现时确认;若只支持单值,批量编辑需走 `metadata_filters` 之外的路径 —— 记入 §9。

### D9. 封面

- 封面数据经 `catalog.resolve_content(book_id, format)` 或 caliberate 的 assets 层取得;实现时确认封面在 service API 中的暴露方式。
- `LibraryBookSummary.has_cover` 给出有无封面。
- 缺封面时显示占位(caliberate 的 egui 侧有 `Generate placeholder cover assets` 能力,见 `docs/roadmaps/roadmap-gui.md`)。
- **caliberate 的自动封面生成是占位渲染器**(`docs/inventory/gui-parity-validation.md`:*"Auto cover generation is a placeholder renderer"*)。GPUI 前端可以做得更好(用 GPUI 画布绘制带文字的封面),但这是**增强**,不属于"复刻 calibre"的范围,列入 §9 的可选增强。

### D10. 美术资源与字体

**决策**:图标用 **calibre 原版**(`../calibre/imgsrc/*.svg`,193 个 + `imgsrc/plugins/`),复制进本仓库 `assets/icons/`;**不引入** zed 的 `assets` crate;字体用 **macOS 系统字体 + CJK fallback**。

- GPUI 原生支持 SVG(`resvg`/`usvg`),**不需要**上游的 `rsvg-convert` 栅格化步骤。
- 主题变体规则:图标名 N 先找 `N-for-{当前主题}-theme.svg`,找不到则用 `N.svg`。calibre 有 `close-for-dark-theme.svg` / `notes-for-dark-theme.svg` / `external-link-for-dark-theme.svg` 以及自动反色的 `modified-for-{theme}-theme.svg`。
- `assets/NOTICE` 记录来源与许可(GPL-3.0-or-later,calibre)。
- 字体:macOS 自带 `PingFang SC` / `Hiragino Sans GB` / `STHeiti`,通过 `FontFallbacks::from_fonts(vec![...])` 挂到 `Font { family: ".SystemUIFont", ... }` 上。**不下载 Noto Sans SC** —— 本机网络不稳(实测 90 秒超时),且 macOS 优先阶段系统字体足够。跨平台阶段再引入 OFL 字体。
- **不**使用 `ui::Icon` 搭配 Zed 的 `IconName` 枚举(那些 SVG 在 `assets` 里);自建图标名空间,名字沿用 calibre 的语义名(`book` / `books_in_series` / `convert` / `search` / `gear` / `metadata` / `remove_books` / `save` / `tags` / `rating` / `reader` / `trash` / `sort` / `layout` / `cover_flow` / `bookshelf` / `fts` / `vl` / `notes` / `toc`),便于与上游逐一对应。

### D11. i18n

**决策**:自实现一个 Fluent 风格的极小 `.ftl` 子集解析器(支持 `key = value` 与 `{ $name }` 占位符),**不引入** fluent 运行时。

理由:消息都是简单占位符,完整 Fluent 运行时较重且带来 API 漂移风险;消息格式保持 Fluent 语法,将来换回真 fluent crate 只需替换内部实现。
> **偏离说明**:第 1 版 spec 写的是 Fluent。本版改为"Fluent 风格子集自实现",记入 §9 偏离清单。

`Locale { ZhCn, En }`,全局 `set_locale`,语言切换后触发 `cx.refresh_windows()`。

### D12. 阅读器(P1,本项目范围内但排在最后)

calibre 的阅读器用 Qt WebEngine(完整 CSS)。GPUI 无 WebView。

**决策**:纯 Rust 文本渲染,`ReaderRenderer` trait 预留 WebView 实现位置。

**依赖缺口**:caliberate 目前**没有** `crates/document`(normalized document model)—— `ARCHITECTURE.md` 把它列为"reader work begins 时新增"的规划;`crates/formats` 只是 parser 占位;`crates/document` 不存在;caliberate 的 reader roadmap 21/21 完成但 parity 审计记录 *"Reader TOC is reconstructed from heading scans instead of source-native TOC structures"*。

**结论**:阅读器需要 caliberate 侧先有 `document` + `formats` 实现(属于 caliberate 的 P1 路线),**不是本前端能独立完成的**。本项目的 Phase I 只做"打开书 → 交给 service 解析 → 渲染最小纯文本"这条通路,具体格式支持等 caliberate 侧就绪。
**EPUB2/EPUB3 是 calibre 侧 reader 的第一优先级**(caliberate `ARCHITECTURE.md` 的 reader priorities:EPUB → HTML → DOCX → PDF),本前端按此顺序对接。

---

## 6. 分阶段路线图

每个阶段的"完成"= 该阶段列出的验收项全部通过,且对应测试通过。

### Phase A —— 依赖打通与骨架

1. 装 `1.98.1` 工具链(走 USTC 镜像)。
2. 建 workspace + 根 `Cargo.toml`(§D1 的 patch + `runtime_shaders`)+ `rust-toolchain.toml` + `.cargo/config.toml` + `LICENSE`。
3. `crates/calibre2rust-gui` 骨架:`main.rs` 调 `gpui_platform::application()`,`theme::init`,自实现 `ThemeSettingsProvider` 与 `AssetSource`,窗口显示中文 + 一个按钮。
4. **service 打通**:`service.rs` 用 `caliberate_app::bootstrap::init(&config)` 拿配置 → `Database::open_with_fts(&config.db, &config.fts)` → `LibraryCatalog::new(&db)` → 列出书数显示在窗口标题上。
5. 复制 calibre 图标进 `assets/icons/`,写 `assets/NOTICE`。

**验收**:窗口打开;中文正常显示(非豆腐块);窗口标题或状态栏显示书库里的书数。
**风险**:caliberate 在 macOS 上的编译结果(见 §7.3);`async-task` patch 缺失;`runtime_shaders` 未开。

### Phase B —— 主窗口骨架

工具栏(calibre macOS 默认动作)、原生菜单栏、搜索栏行、5 面板 + 4 分隔条(10px 隐藏 / 50px 恢复)、宽/窄布局切换(Alt+Shift+L)、状态栏。

**验收**:面板可拖动、可隐藏/恢复;布局切换生效;工具栏与 calibre macOS 默认一致(无 Help/Preferences)。

### Phase C —— 书列表(表格视图)

`ui::Table` + `query_summary_page` 分块加载;列定义(默认隐藏 `last_modified/languages/formats/id/path/pages`);单级排序 + id tie-breaker;列宽可调 + 持久化;列可见性;多选;行内快速搜索高亮。

**验收**:10 万本书的库滚动不卡;排序/列宽/列可见性重启后保持。

### Phase D —— 分类浏览器(左栏)

6 类 facet(Authors/Tags/Series/Publishers/Ratings/Languages),带计数;点击 → `metadata_filters`;再次点击取消;Exclude 模式;与书列表联动。

**验收**:点作者 → 书列表只剩该作者的书;计数正确;与 calibre 行为一致。

### Phase E —— 搜索栏

自由文本搜索(全字段)+ 字段定向(`title:` / `author:` / `tag:` / `series:` / `publisher:` / `format:`);与 facet 过滤 AND 组合;结果计数;清空。

**验收**:搜索 + facet 过滤组合正确;空结果显示友好的空状态而非空白。

### Phase F —— 书籍详情(右栏)

封面 + 元数据表 + 格式列表 + 评论;值可点击;无封面时的占位;空选择时的空状态。

### Phase G —— 封面网格视图 + 视图切换

网格与表格共享数据源与选择状态;切换持久化。

### Phase H —— 元数据编辑

单本编辑对话框 + 批量编辑;字段覆盖 calibre 基本字段;保存后书列表与详情即时刷新。

### Phase I —— 工具栏动作与首选项

添加书籍(文件/文件夹) / 删除书籍 / 转换书籍 / 保存到磁盘 / 虚拟库(保存搜索) / 首选项对话框;视图状态(面板尺寸、列宽、视图模式)持久化。

### Phase J —— 阅读器(最小通路)

打开书 → caliberate 的 format/document 层(若已就绪)→ 纯文本渲染;目录导航;阅读位置持久化。**依赖 caliberate 侧 `crates/document` + `crates/formats` 就绪**,未就绪则此阶段顺延并记入 §9。

---

## 7. 验证方法

### 7.1 前端行为对拍(与 calibre GUI)

用同一书库,在 Python calibre GUI 与 GPUI 前端各做一遍相同操作,比对**结果集合**(不是像素):
- 同一 facet 点击 → 相同的书 ID 集合;
- 同一搜索词 → 相同的书 ID 集合;
- 同一排序字段 + 方向 → 相同的书序;
- 同一元数据编辑 → 落库后 `metadata.db` 中该书字段相同。

### 7.2 Rust 侧测试

- `service.rs` 的查询构造是纯函数 → 单测:facet 选择 → `LibraryQuery` 的 `metadata_filters`;搜索词 → `LibraryQuery` 的字段。
- 视图状态(面板尺寸、列宽、列可见性)的序列化往返 → 单测。
- 持久化的 clamp 行为:窗口变小后,恢复的旧面板尺寸不得把书列表挤没 → 单测(caliberate 的 philosophy 明确要求:"Their saved geometry must be clamped against the current window")。
- GPUI 的 `VisualTestAppContext` 在 macOS 上需主线程 → 标记 `#[ignore]`,手动跑;其余逻辑抽成纯函数。

### 7.3 macOS 兼容性验证(caliberate 侧)

**必须在 Phase A 完成前完成**:`cargo check --workspace` 在 macOS 上是否通过。caliberate 官方保证 Windows/Linux,macOS 未验证。
- 若 caliberate 某些 crate 在 macOS 编译失败(尤其 `crates/device` 的挂载检测、`crates/metadata` 的 `unrar`),则**只消费通过编译的 crate**(core/db/library/assets),把有问题的 crate 排除在外。
- 验证命令:`cargo check -p caliberate-core -p caliberate-db -p caliberate-library -p caliberate-assets`(在 `../caliberate` 下跑)。

### 7.4 构建与质量门禁

```bash
cargo build --target aarch64-apple-darwin
cargo fetch --target aarch64-apple-darwin
cargo test --target aarch64-apple-darwin
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

遵循 zed 的 Rust 规范(来自 `zed/AGENTS.md`):不用 `unwrap()`(用 `?` 传播)、不静默丢弃错误(`let _ =` 禁止用于可失败操作)、不写"总结代码"的注释、完整拼写变量名、**不建 `mod.rs`**、新 crate 用 `[lib] path = "src/<name>.rs"`。
沿用 caliberate 的约定:用 `tracing` 而非 `println!` 做运行时诊断。

---

## 8. 已拍板的决策

| # | 决策 | 拍板 | 理由 |
|---|---|---|---|
| 1 | GPUI 依赖方式 | git + 固定 rev `92447a17` | 用户明确要求;crates.io 版本 API 过时 |
| 2 | `async-task` patch | 必须在消费方根 Cargo.toml 重复声明 | cargo 忽略依赖内的 patch |
| 3 | Metal shader | 启用 `runtime_shaders` | 本机无 Metal Toolchain |
| 4 | 工具链 | pin `1.98.1` | zed 的要求;直连 rust-lang.org 超时,须走镜像 |
| 5 | **与 caliberate 的关系** | **复用后端,只写 GPUI 前端** | **用户已拍板**。避免重复 3 万行;契合 caliberate 的 service/adapter 架构 |
| 6 | **目标平台** | **macOS 优先** | **用户已拍板**。caliberate 官方保证 Win/Linux,macOS 需自行验证 |
| 7 | 依赖 caliberate 的方式 | git 依赖,固定 rev `f9c41d7` | 可复现;caliberate 无 patch 段,消费无额外成本 |
| 8 | crate 数量 | 只写 1 个 crate | 前端预计 5–8k 行,拆多 crate 只增 ceremony |
| 9 | 后端访问边界 | 只有 `service.rs` 接触 caliberate | caliberate 架构要求前端不重复实现查询/搜索/路径逻辑 |
| 10 | 书列表数据源 | `query_summary_page` 分块 | 大库必须分页;`LibraryBookSummary` 字段正好够用 |
| 11 | 分类浏览器 | 用 `LibraryFacetKind` 的 6 类 | 不自造 facet 体系;Formats/Identifiers 是 service 缺口 |
| 12 | 搜索 | 第一版仅自由文本 + 字段定向 | calibre 完整搜索语法是 service 缺口 |
| 13 | 图标 | calibre 原版 `imgsrc/*.svg` | **用户已拍板**;GPUI 原生支持 SVG,无需栅格化 |
| 14 | 字体 | macOS 系统字体 + CJK fallback | 本机网络不稳;macOS 优先阶段不需要下载 |
| 15 | 许可 | 整仓 GPL-3.0-or-later | calibre GPLv3 + caliberate/zed ui 均 GPL-3.0-or-later |
| 16 | i18n | Fluent 风格子集自实现 | 消息简单,避免重依赖;格式保持 Fluent 语法 |
| 17 | 阅读器 | 纯 Rust 文本渲染,排在 Phase J | 依赖 caliberate 侧 `document`/`formats` 就绪 |

---

## 9. 未决问题与缺口清单

### 9.1 需要人来拍板的

1. **caliberate 的 git 依赖 rev 是否随开发推进**
   建议:开发期用 `path` 依赖联调,提交前换回固定 rev;caliberate 有实质进展后再 bump。
2. **是否最终并回 caliberate 仓库**(作为 `crates/gui-gpui`)
   本项目先独立;等 GPUI 前端稳定后再议。需要架构师决定。
3. **是否要求 GUI 与 caliberate 的 egui 前端功能对等**
   egui 侧 16,876 行已覆盖大量细节(快捷键、偏好项、拖拽、图标主题)。完全对等是巨量工作;建议按 §6 的阶段推进,每个阶段以"calibre 的对应功能可用"为验收,而不是"egui 侧有多少行就做多少行"。

### 9.2 caliberate 的 service 缺口(记入 caliberate issue 清单,不在本前端绕过)

| 缺口 | 影响 | 前端的临时做法 |
|---|---|---|
| 无 calibre 完整搜索语法(`= ~ ^ \` `=. =..` 三引号 日期 评分 三态 `search:` `vl:` `@UserCategory:`) | 无法提供 calibre 等价的搜索 | 只做自由文本 + 字段定向 |
| 无 `LibraryFacetKind::Formats` / `Identifiers` | 分类浏览器少 2 类 | 不显示这两类 |
| `LibraryQuery` 只支持单级排序 | 无法做 calibre 的多级排序 | 主字段 + id tie-breaker |
| `LibraryBackend` 无写操作(只有读) | 元数据编辑需绕到 `Database` | `service.rs` 直接调 `Database::update_book_*`,但仍不碰 SQL |
| `update_book_*` 是否支持多值(标签/系列/语言)未确认 | 批量编辑可能受阻 | 实现时确认,必要时记为缺口 |
| 封面在 service API 中的暴露方式未确认 | 详情面板取封面 | 实现时确认 `resolve_content` / assets 层 |
| 无 `crates/document`,`crates/formats` 仅占位 | 阅读器无输入 | Phase J 顺延 |
| 自动封面生成是占位渲染器 | 封面质量差 | 前端可用 GPUI 画布增强(可选) |

### 9.3 与第 1 版 spec 的偏离

| 项 | 第 1 版 | 本版 | 理由 |
|---|---|---|---|
| 数据层 | 自建 db + 搜索 + 导入器 | 复用 caliberate | 用户拍板 |
| crate 数 | 4 个 | 1 个 | 前端规模不足以拆 |
| i18n | Fluent crate | Fluent 风格子集自实现 | 避免重依赖 |
| 字体 | 打包 Noto Sans SC | macOS 系统字体 + fallback | 网络不稳,macOS 优先 |
| 格式层(自建 EPUB/MOBI 等) | 自建 | 复用 caliberate `crates/formats`(占位) | 重复造轮子;阅读器待 caliberate 就绪 |
| PDF | 暂不做留占位 | 不在本前端范围 | 属 service 侧 |
| EPUB2/EPUB3 优先 | 是 | 保留(Phase J 对接顺序) | 与 caliberate reader priorities 一致 |

---

## 10. 坐标索引

**calibre(Python,行为基准)** —— 相对 `../calibre`:

| 主题 | 位置 |
|---|---|
| 主窗口布局 | `src/calibre/gui2/central.py`, `layout.py` |
| 工具栏 / 菜单栏默认值 | `src/calibre/gui2/__init__.py:331`(menubar)、`:359-376`(macOS toolbar) |
| 分类浏览器 | `src/calibre/gui2/tag_browser/`(`ui.py`, `view.py`, `model.py`) |
| 书列表 | `src/calibre/gui2/library/views.py`, `models.py` |
| 封面网格 / 书架 | `src/calibre/gui2/library/alternate_views.py`, `bookshelf_view.py` |
| 书籍详情 | `src/calibre/gui2/book_details.py` |
| 搜索框与限制 | `src/calibre/gui2/search_box.py`, `search_restriction_mixin.py` |
| 搜索语言语义 | `src/calibre/db/search.py`;词法 `src/calibre/utils/search_query_parser.py` |
| 字段元数据 | `src/calibre/library/field_metadata.py` |
| title_sort / author_sort | `src/calibre/ebooks/metadata/__init__.py:67`, `:203` |
| 搜索语法手册 | `manual/gui.rst:380-400`(`=.` / `=..` 权威示例) |
| 图标源 | `imgsrc/*.svg`(193 个) |
| 格式插件 | `src/calibre/ebooks/conversion/plugins/` |
| EPUB2→3 升级 | `src/calibre/ebooks/oeb/polish/upgrade.py` |

**caliberate(后端)** —— 相对 `../caliberate`:

| 主题 | 位置 |
|---|---|
| service facade / backend trait | `crates/library/src/catalog.rs` |
| 查询 / facet / 排序 | `crates/library/src/query.rs` |
| 列表摘要 | `crates/library/src/summary.rs` |
| calibre 库只读兼容 | `crates/library/src/calibre/` |
| SQLite 持久化 | `crates/db/src/database.rs` |
| 配置模型 | `crates/core/src/config.rs`(`ControlPlane`) |
| 引导 | `crates/app/src/bootstrap.rs` |
| egui 前端(参考实现) | `crates/gui/src/views.rs`(`LibraryCatalog::new(&db)` 用法见 `:90`, `:8461`, `:8499`) |
| GUI parity 缺口 | `docs/inventory/gui-parity-validation.md` |
| 架构边界 | `ARCHITECTURE.md` |
| 产品优先级 | `docs/project/priorities.md` |

**GPUI** —— 相对 `../zed`:

| 主题 | 位置 |
|---|---|
| 应用启动 | `crates/gpui/examples/hello_world.rs`, `crates/gpui_platform/src/gpui_platform.rs` |
| 输入框参考实现 | `crates/gpui/examples/input.rs` |
| 表格组件 | `crates/ui/src/components/data_table.rs` |
| 弹窗 / 菜单 / 提示 | `crates/ui/src/components/` |
| actions / keymap | `crates/gpui/src/action.rs`, `keymap/` |
| 拖拽 | `crates/gpui/examples/drag_drop.rs` |
| 富文本 | `crates/gpui/src/elements/text.rs`(`StyledText` / `TextRun` / `TextLayout`) |
| 资源接口 | `crates/gpui/src/assets.rs`(`AssetSource`) |
| 主题初始化 / provider | `crates/theme/src/theme.rs`(`theme::init`), `theme_settings_provider.rs` |
| 树形行组件 | `crates/ui/src/components/tree_view_item.rs` |
| Metal shader 编译 | `crates/gpui_apple/build.rs`(`runtime_shaders` 分支 `:106-121`) |
