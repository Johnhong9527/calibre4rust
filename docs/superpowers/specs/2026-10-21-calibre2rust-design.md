# calibre2Rust 设计文档

> 日期:2026-10-21
> 状态:**待批准**
> 参考实现:`../calibre`(calibre Python 源码树,`src/calibre/`,9.16.100 / commit `43b5c26`)
> UI 框架来源:`../zed`(zed 仓库,`92447a17cc008225c73619f1c3f0cdefc8ababf3`,branch `main`)

> **评审记录**:本文档经一轮"实现者视角"独立评审,评审方实际编译并运行了 GPUI 栈,发现并修正了 5 处事实性错误(搜索运算符 `=.` 顺序、默认排序方向、macOS 工具栏默认项、格式插件目录位置、crate 循环依赖)与 1 处环境级阻塞(缺 Metal Toolchain)。所有修正已合入本文档,并在对应章节标注了上游 file:line 依据。
>
> **用户拍板(第 2 轮)**:**EPUB2 与 EPUB3 为优先支持格式**;**许可证 = GPL-3.0-or-later**;**PDF 暂不做,留占位符**;**图标使用 calibre 原版**。均已写入 §8 决策 26–29。

---

## 1. 意图与动机

在 Rust 中重建 calibre:以 GPUI 作为 UI 框架,逐功能对齐上游 calibre 的**界面与操作流程**。

- 上游 calibre 的 GUI 自 5.0 起是 **toolbar-driven**(不再是 File/Edit/View 菜单结构),主窗口由 5 个面板 + 4 条可拖拽分隔条组成,主菜单是 macOS 原生菜单栏。
- 本项目不追求 SQLite 层的字节级兼容,但**语义必须一致**:搜索表达式、字段体系、排序规则、title/author sort 算法、元数据模型、格式读写行为、目录约定,都要与 calibre 对齐,让用户"用起来像 calibre"。
- 数据层另起炉灶(新 schema),但保留 calibre 的磁盘目录约定,便于存量书库迁移进来。

**为什么用 GPUI**:GPUI 是 Zed 的 UI 框架,原生 macOS 渲染、与 Rust 后端同构、支持多窗口/拖拽/富文本/虚拟滚动列表,已在生产环境验证。本项目把 `gpui` 直接作为 **git 依赖**引入,跟随 zed 主线演进。

---

## 2. 目标 / 非目标 / 反目标

### 2.1 目标(按交付顺序)

| # | 目标 | 验收标准 |
|---|---|---|
| G1 | GPUI 以 git 依赖方式可用 | `cargo build` 通过,能弹出窗口渲染文本 |
| G2 | 书库内核:字段系统、存储、搜索、排序 | 与 calibre 的搜索/排序结果逐条对拍一致 |
| G3 | 主界面:标签浏览器 + 书列表 + 书籍详情 + 搜索栏 | 面板结构、交互、快捷键与 calibre 一致 |
| G4 | 导入:单文件 / 多选 / 文件夹 / 从既有 calibre 书库 | 能把存量书库完整搬进来,元数据不丢 |
| G5 | 元数据编辑:单本 + 批量 | 字段覆盖 calibre 的基本字段 + 自定义列 |
| G6 | **EPUB2 / EPUB3 优先支持**(P0) | 两版都能读元数据、写元数据、转换、渲染;元数据与封面往返无损 |
| G7 | 格式层:其余格式的元数据模型 + OEB 中间表示 + 主流格式读写 | TXT/HTML/HTMLZ/FB2/MOBI/AZW3/RTF/ODT(PDF 暂留占位符) |
| G8 | 转换管线 + `ebook-convert` 等价 CLI | 覆盖 calibre 的 pipeline 选项 |
| G9 | 阅读器:纯 Rust 文本渲染 + 目录 + 注释/书签 | EPUB2/EPUB3/FB2/TXT 可读,带章节导航与高亮 |
| G10 | 内容服务器 + OPDS + `calibredb` 等价 CLI | 浏览器可访问、可检索、可下载 |
| G11 | 中英双语界面可切换 | 设置中切换,立即生效 |

### 2.2 非目标(本项目不做)

- 嵌入式 Python 运行时,不兼容 calibre 的 Python 插件生态。
- DRM 书籍:AZW/KFX/PDF-DRM/LIT-DRM 一律报 `DrmError`。
- ZEBRA 格式 —— calibre 上游本身就不支持。
- 跨平台 GUI:一期只做 macOS。数据层与格式层保持跨平台(不引入平台专属 API),为后续 Linux/Windows 留路。
- **PDF 转换**:暂不做,格式注册表里只留占位插件(§8 决策 27)。
- 不追求与 calibre 的 `metadata.db` 二进制兼容。

### 2.3 反目标(做成了也算失败)

- **UI 骨架先行、后补功能**:一个只有空表格的壳子没有价值。每个阶段必须端到端可用。
- **"大概能用"的搜索**:搜索语义含糊(比如大小写/重音/分词处理不一致)会让用户以为书库坏了,比不支持更糟。
- **静默失败**:任何解析错误、导入失败、格式不支持,必须给用户可见的中文/英文提示,不能吞掉。
- **一次性大爆炸**:不接受 8000 行的单文件 GUI 提交。

---

## 3. 约束

### 3.1 硬约束(来自环境与上游)

| 约束 | 细节 |
|---|---|
| Rust 工具链 | zed 要求 `1.98.1`(`zed/rust-toolchain.toml`)。本机当前仅 `stable(1.92)` 与 `1.93` → **必须先 `rustup toolchain install 1.98.1`**(该版本确实存在,已验证) |
| 工具链下载 | 直连 `static.rust-lang.org` 在本机会超时,需走镜像:`RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1` |
| Edition | zed workspace 为 `edition = "2024"`,本项目跟随 |
| **Metal Toolchain 缺失** | 本机 `xcrun metal --version` 报 *missing Metal Toolchain* —— 它是 Xcode 的独立可下载组件(数 GB),**不在 CLT 里**。`gpui_apple/build.rs:161-181` 会 `process::exit(1)` 直接构建失败。**解法:启用 `gpui_platform` 的 `runtime_shaders` 特性**,让 shader 在运行时由 Metal framework 编译,完全跳过 `xcrun metal`。备选:`xcodebuild -downloadComponent MetalToolchain`(本项目不采用) |
| macOS 构建 | 需完整 Xcode(已安装 3.9 GB)。`cmake` 本机缺失,当前不依赖 |
| `MACOSX_DEPLOYMENT_TARGET` | zed 设 `10.15.7`。注意:`gpui_apple/build.rs:137` 的 `-mmacosx-version-min=10.15.7` 是传给 **`xcrun metal`** 的,不是传给 rustc 的;rustc 侧的 target 由 `.cargo/config.toml` 的 `[env]` 决定,两者独立,照抄 zed 的 `[env]` 块即可 |
| Cargo patch 继承 | **`[patch.crates-io]` 只在消费方 workspace 根生效**,依赖内的 patch 会被忽略 |
| 网络 | crates.io 下载不稳定(常见 `spurious network error` / timeout)。`cargo metadata` 会解析**所有 target** 并拖入 windows/linux/wasm 依赖,极易卡死 → **用 `cargo build --target aarch64-apple-darwin` 或 `cargo fetch --target` 代替 `cargo metadata`** |
| GPL 许可 | calibre 为 GPLv3。`ui`/`theme`/`icons` 等 zed crate 同样是 GPL-3.0-or-later。本项目为 calibre 衍生实现,分发时需按 GPLv3 处理(见 §9) |

### 3.2 GPUI 的能力边界(调研结论,直接影响设计)

| 事实 | 影响 |
|---|---|
| crates.io 上的 `gpui 0.2.2` 是**旧 API**(`Application::new()`、bundled blade-graphics) | **必须用 git + rev**,不能用 crates.io 版本 |
| 当前 zed main 的入口是 `gpui_platform::application()`(`gpui::Application::with_platform`) | 启动代码按此写 |
| `gpui` 依赖 `gpui_platform` 且 `gpui_platform` 的 `default = ["multithreaded"]` → 需要 nightly | 消费方必须 `default-features = false` |
| zed 用了 `async-task` 的 fork(把 API 泛型化成 `Task<T, M>`、`Runnable<M>`) | **必须在我们的根 Cargo.toml 重复 patch `async-task`**,否则编译失败 |
| gpui **没有** tree 组件 | 标签浏览器树要自己实现(缩进 + 展开态 + 虚拟列表)。`ui::TreeViewItem` 可复用行渲染(展开/折叠/选中/焦点),但数据模型与虚拟化仍需自建 |
| gpui **没有** input 组件 | 单行输入框参考 `zed/crates/gpui/examples/input.rs`(920 行)自行实现。**注意**:Zed 确实有 `ui_input::InputField`,但它要求 `ERASED_EDITOR_FACTORY`,而唯一的 setter 在 `editor` crate 里 → 引入 `ui_input` 等于引入 Zed 整个文本编辑器栈(workspace/project/lsp/dap/rpc)。**明确拒绝**,理由记录在案以免日后反复讨论 |
| gpui **没有** table 组件 | 用 `ui::Table`(`zed/crates/ui/src/components/data_table.rs:356`),它提供可调列宽、列筛选、斑马纹、`uniform_list` 虚拟滚动 |
| **`ui` 组件在没有 `ThemeSettingsProvider` 时会 panic** | `theme::theme_settings(cx)` 在未注册 provider 时直接 panic(`crates/theme/src/theme_settings_provider.rs:41`),而 `ui::Table` 默认 `use_ui_font: true` 会调用它 → **必须自己实现该 trait(5 个方法:`ui_font` / `buffer_font` / `ui_font_size` / `buffer_font_size` / `ui_density`,约 40 行)**,以免引入 zed 的 `theme_settings` crate(它会拖入 `settings` → `fs` → `notify`,并需要再补两个 `[patch.crates-io]` 条目) |
| `theme` 需要显式初始化 | 任何 `ui` 组件渲染前必须 `theme::init(theme::LoadThemes::JustBase, cx)` |
| `icons` crate 只提供图标名枚举,不含图片 | `ui::Icon` 通过 `IconName::path()` → `"icons/<stem>.svg"` 向 `AssetSource` 取图 |
| `assets` crate = 4.6 MB 的 `fs_embed!`(1.6 MB 字体 + 1.6 MB 图标),依赖只有 `anyhow`/`gpui`/`util` | **不是"体积巨大"**。但它是 GPL-3.0 且**不含中文字体** → 见 D12 的决策:自建 `AssetSource` + 自带 OFL 中文字体,`icons` 只借用名称枚举不复用其 SVG |
| `text` crate 是协作式 rope buffer,不是富文本 | 富文本用 `gpui::StyledText` + `TextRun` + `TextLayout` |
| `gpui_platform` 的 `multithreaded` 只影响 wasm | 对 macOS 构建无影响;`default-features = false` 仍照抄 zed(`zed/Cargo.toml:375`)。`font-kit` 在 macOS 上**必须开**(`gpui_apple/src/gpui_apple.rs:14-20` 用它 gate 整个 Apple 文本系统) |

### 3.3 依赖策略

- **最小化**:能用标准库/已有依赖就不新增。`zed` 官方规则同样要求"避免不必要的外部包"。
- 允许的第三方(按需引入,每个都要有理由):`rusqlite`(bundled SQLite)、`serde`/`serde_json`、`chrono`、`regex`、`zip`、`quick-xml`、`flate2`、`image`、`html5ever`+`markup5ever`、`icu`(collator/casemap/datetime/locale)、`clap`、`axum`(后期)、`anyhow`/`thiserror`、`uuid`、`sha2`。
- **不引入**:Python 运行时、Electron/Qt/WebView(阅读器走纯 Rust)、重量级 ORM。

---

## 4. 整体架构

### 4.1 crate 划分

```
calibre2Rust/
├── Cargo.toml                      # workspace 根,含 [patch.crates-io] async-task
├── rust-toolchain.toml             # channel = "1.98.1"
├── .cargo/config.toml              # MACOSX_DEPLOYMENT_TARGET
├── crates/
│   ├── calibre-core/               # 纯 Rust:元数据模型/字段/存储/搜索/排序/模板/导入导出。无 GPUI
│   ├── calibre-formats/            # 纯 Rust:格式读写插件/OEB 中间表示/转换管线。依赖 core。无 GPUI
│   ├── calibre-i18n/               # 纯 Rust:中英文案目录与查表 API
│   ├── calibre-app/                # GPUI 界面 + 主二进制 calibre2rust
│   └── calibre-server/             # 内容服务器(axum)+ OPDS,后期
├── assets/                         # 自有图标 SVG + 中文字体(OFL)
├── i18n/                           # zh-CN.ftl / en.ftl
└── docs/superpowers/specs/
```

**为什么这样切**:`calibre-core` 与 `calibre-formats` 不依赖 GPUI,可以脱离 GUI 单独跑测试与 CLI,也能被将来的 TUI/CLI 复用。GPUI 编译极慢(见 §4.4),把它隔离在最上层可以让核心逻辑的测试迭代不触发 UI 重编。

**依赖方向必须单向**:`calibre-core` ← `calibre-formats` ← `calibre-app`。
关键约束:**`Metadata`(元数据模型)属于 `calibre-core`**,不属于 `calibre-formats`。
- `calibre-core` 需要 `Metadata`:它要写 `metadata.json` 边车、要存书库字段、要被 `calibre-formats` 读写。
- `calibre-formats` 需要 `Metadata`:格式插件的 `get_metadata` / `set_metadata` 收发它,并负责在 `Metadata` ↔ `OEBBook` 之间转换。
- 若把 `Metadata` 放在 `calibre-formats`,则 `calibre-core` 要么依赖 `calibre-formats`(与"core 不含格式逻辑"的划分冲突),要么只能依赖一个 trait(导致导入路径必须绕一圈)。**因此 `Metadata` 归 core,`OEBBook` 归 formats。**

### 4.2 依赖图

```
calibre-app ──► calibre-core ──► rusqlite / serde / icu / regex / chrono
     │         └► calibre-formats ──► calibre-core / zip / quick-xml / html5ever / image
     │         └► calibre-i18n
     └──────────► gpui(git) / gpui_platform(git) / ui(git) / theme(git) / icons(git)
                   / menu(git) / fuzzy(git) / component(git) / gpui_macros(git) / gpui_util(git)
calibre-server ──► calibre-core + calibre-formats + axum
```

无环:`core` 不依赖 `formats`;`formats → core`;`app → {core, formats}`。

### 4.3 关键类型流

```
用户操作(GPUI)
   └─► AppState(Entity)  ──► LibraryService(calibre-core)
                                   └─► Library(读写 SQLite + 磁盘目录)
                                          ├─ FieldRegistry(字段元数据)
                                          ├─ SearchQuery(AST) ──► SQL 编译器
                                          └─ Metadata(元数据模型,归 core)
                                                 ▲
                                                 │ calibre-formats 提供
                                          FormatRegistry(格式插件)
                                                 └─ OEBBook(OEB 中间表示,归 formats)
```

`calibre-core` 依赖 `FormatRegistry` 抽象(一个 trait)而非 `calibre-formats` 的具体实现;`calibre-app` 在启动时把 `calibre-formats` 的实现注入。

### 4.4 构建成本预估

GPUI 全家桶(`gpui` + `gpui_macros` + `gpui_platform` + `gpui_macos` + `gpui_apple` + `ui` + `theme` + `icons` + `menu` + `fuzzy` + `component`)依赖图约 **665 个包**(其中 22 个是 zed crate)。**zed 的 git 依赖本身是一个 412 MB 的 bare clone**(`~/.cargo/git/db/zed-*`)。

- 冷缓存首次编译:**30–60 分钟**(本机网络不稳,需重试)。
- 增量 `cargo check`:**30–90 秒**。
- 依赖拉取用 `cargo fetch --target aarch64-apple-darwin`;**不要用 `cargo metadata`**(会解析所有 target,本机实测 >15 分钟未完成)。

**开发节奏必须围绕这一点设计**:核心逻辑放 `calibre-core` 里用 `cargo test -p calibre-core` 快速迭代,UI 改动才触发 GPUI 重编。

---

## 5. 关键设计决策

### D1. GPUI 以 git 依赖引入

**决策**:在 workspace 根 `Cargo.toml` 中用 `git = "https://github.com/zed-industries/zed"` + `rev = "92447a17cc008225c73619f1c3f0cdefc8ababf3"` 引入 `gpui` 及其同伴 crate。

**理由**:
1. 用户明确要求"直接引用 zed 仓库内 gpui crates 作为 git 依赖"。
2. crates.io 上的 `gpui 0.2.2` 是旧 API(`Application::new()`),与当前 zed main 的 `gpui_platform` 架构不兼容,用了就编译不过。
3. 本地 `../zed` 是官方仓库的干净 clone,该 commit 在 `origin/main` 上 → git 源可解析。

**必须复制的 zed 根 manifest 设置**(消费方根 `Cargo.toml`):

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

# 关键:cargo 只读取消费方 workspace 根的 patch,依赖内的 patch 会被忽略。
# zed 使用了 async-task 的 fork(把 API 泛型化为 Task<T, M> / Runnable<M>),
# 缺少这个 patch 会报类型不匹配错误。
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

**关于 `runtime_shaders`**:本机没有 Metal Toolchain,`xcrun metal` 不可用。启用 `gpui_platform` 的 `runtime_shaders` 特性后,`gpui_apple/build.rs:106-121` 走 `emit_stitched_shaders` 路径 —— 把 shader 源码与生成的头文件拼成一个自包含的 `.metal` 文件,交由 Metal framework **在运行时**编译,完全绕开 `xcrun metal` 调用。

**风险**:zed main 是移动靶,`rev` 锁定解决了漂移;但若未来要升级,需重新核对 §3.2 的所有边界条件。

**验证状态(2026-10-21,已实测)**:在临时目录用上述 manifest 完整编译了 GPUI 栈并运行 —— 编译通过、进程启动、事件循环存活、**无任何 Metal/shader 错误输出**。
**尚未验证**:像素是否正确渲染 —— 本机 `screencapture` 与 `osascript` 均被系统权限(录屏/辅助功能)拒绝,无法截图。**Phase 0 必须在有录屏权限的机器上做一次肉眼确认**。

### D2. 数据层:新 schema,calibre 语义

**决策**:SQLite(rusqlite,bundled,WAL 模式),表结构自定但与 calibre 的**表名与列语义对齐**(便于迁移与理解),元数据边车文件用 `metadata.json`(自描述、易调试),磁盘目录沿用 calibre 约定。

**Schema 草案**:

```sql
books(id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL,
      title_sort TEXT NOT NULL,
      author_sort TEXT NOT NULL,
      series_index REAL NOT NULL DEFAULT 1.0,
      pubdate TEXT,                 -- ISO8601
      timestamp TEXT NOT NULL,      -- 导入时间
      last_modified TEXT NOT NULL,
      uuid TEXT NOT NULL,
      has_cover INTEGER NOT NULL DEFAULT 0,
      size INTEGER NOT NULL DEFAULT 0,
      path TEXT NOT NULL,           -- 相对库根
      pages INTEGER)
authors(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, sort TEXT NOT NULL)
tags(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)
series(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)
publishers(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)
ratings(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)   -- 0..10
languages(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)
comments(book INTEGER PRIMARY KEY, text TEXT NOT NULL)
identifiers(book INTEGER NOT NULL, scheme TEXT NOT NULL, val TEXT NOT NULL)
data(book INTEGER NOT NULL, format TEXT NOT NULL, name TEXT NOT NULL,
     uncompressed_size INTEGER NOT NULL DEFAULT 0, mtime TEXT)
books_authors_link(book INTEGER NOT NULL, author INTEGER NOT NULL)
books_tags_link(book INTEGER NOT NULL, tag INTEGER NOT NULL)
books_series_link(book INTEGER NOT NULL, series INTEGER NOT NULL, extra REAL DEFAULT 1.0)
books_publishers_link(book INTEGER NOT NULL, publisher INTEGER NOT NULL)
books_ratings_link(book INTEGER NOT NULL, rating INTEGER NOT NULL)
books_languages_link(book INTEGER NOT NULL, language INTEGER NOT NULL)
books_pages_link(book INTEGER NOT NULL, pages INTEGER NOT NULL)
custom_columns(id INTEGER PRIMARY KEY, label TEXT NOT NULL, datatype TEXT NOT NULL,
                is_multiple INTEGER NOT NULL DEFAULT 0, display TEXT)
custom_column_<n>(...)              -- 归一化类型:值表 + books_custom_column_<n>_link
                                   -- 非归一化类型:每书一行
preferences(key TEXT PRIMARY KEY, value TEXT)   -- 保存的搜索/虚拟库/插件设置
annotations(id INTEGER PRIMARY KEY, book INTEGER NOT NULL, format TEXT,
            type TEXT, timestamp TEXT, start_cfi TEXT, end_cfi TEXT,
            highlighted TEXT, note TEXT, style TEXT)
books_fts                          -- FTS5 虚表,独立于元数据搜索语言
```

**磁盘布局**(沿用 calibre 约定,便于存量迁移):

```
<library>/
├── calibre.db                     # 主库
├── .notes.db                      # 注释/书签
├── .trash/b/<book_id>/            # 整书回收站
├── <作者>/<书名> (<id>)/
│   ├── <书名> - <作者>.<ext>      # 格式文件
│   ├── cover.jpg
│   ├── metadata.json              # 元数据边车(替代 calibre 的 metadata.opf)
│   └── data/                      # 附加文件
```

**与 calibre 的差异(有意为之)**:边车用 JSON 而非 OPF2(更简单、更适合 Rust 序列化、便于调试);不实现 calibre 的 `books_plugin_data`、`library_id`、`metadata_dirtied` 等历史表;不实现 `schema_upgrades` 的版本迁移链(新库从零建)。

**取舍**:放弃 OPF2 意味着本项目产出的书库不能被 calibre 直接打开。接受这个代价(用户已确认"数据层可以另起炉灶")。

### D3. 搜索表达式语言

**决策**:完整实现 calibre 的元数据搜索语言,语义**逐条对齐上游(bit-exact parity)**。

> **对拍标准(拍板)**:§7.1 的对拍门禁要求**与上游逐位一致**,包括上游的怪癖。实现者不得"顺手修正"上游的怪异行为(例如 `field:` 空值匹配不到任何东西、评分用整除、`"""` 三引号内不做转义处理)。凡是本项目与上游不一致的地方,必须是**有意为之且已记录在案**。

**词法**(`utils/search_query_parser.py`):
- 裸词规则:`[^"()\s]+`;另有独立规则 `@.+?:[^")\s]+` 专门匹配 `@UserCategory:item`。
- 引号串:`"..."`,`\"` 可转义。
- **三引号 `"""..."""`**:第二种引号模式,**内部不做任何转义处理**。上游先把 `"""..."""` 整体转成十六进制再词法分析(`search_query_parser.py:193-199`)。写正则、或写 `tags:"=..Military.WWII"` 这类含点号的层级查询时必须用它。
- 转义:`\\`、`\"`、`\(`、`\)`。
- 括号 `(` `)` 为独立 token。

**语法**:
```
prog        ::= or_expr
or_expr     ::= and_expr ('or' or_expr)?
and_expr    ::= not_expr ('and'? and_expr)?      -- 相邻项隐式 AND
not_expr    ::= 'not'? location_expr
location_expr ::= base_token | '(' or_expr ')'
```

**匹配符**(`db/search.py:36-55` `_matchkind`)。**顺序至关重要** —— `=` 必须在前,层级前缀在 `=` **之后**:

| 写法 | 语义 |
|---|---|
| 默认 | ICU 主级"包含"(忽略大小写、去重音、去标点);ASCII 走快路径 |
| `field:=value` | 精确匹配(字符变体敏感:`é` ≠ `e`) |
| `field:\value` | 字面包含(先剥掉 `\`,再走包含匹配) |
| `field:~regex` | 正则匹配(Python `regex` 语法) |
| `field:^value` | 去重音匹配 |
| `field:=.value` | **层级前缀**:`value` 是整个点分项的**前缀**(如 `tags:"=.History"` 匹配 `History.Military.WWII`) |
| `field:=..value` | **层级分量**:`value` 精确等于点分项的**某一段**(如 `tags:"=..WWII"` 匹配 `History.Military.WWII`,但 `tags:"=..Military.WWII"` 不匹配) |

> ⚠️ 常见错误:写成 `field:.=value` 是**错的**。`_matchkind` 只识别 `\` `=` `~` `^` 四个前缀,遇到前导 `.` 会退化成"字面包含 `.=value`",静默匹配不到任何东西。上游手册 `manual/gui.rst:390-400` 明确写的是 `=.` 与 `=..`。
>
> 层级语义的实现要点(`db/search.py:57-91`):`..` 分支下 `query == t` 或 `query` 是 `t.split('.')` 去掉空白后的某一段;`.` 分支下 `t.startswith(value)` 且其后紧跟 `.` 或字符串结束。

**长度保护**:`_matchkind` 只在 `len(query) > 1` 时才剥前缀。所以 `field:=`(查询串就是 `=`)不是精确匹配,而是"包含 `=`";`field:\` 是"包含反斜杠"。

**布尔**:接受 `yes/no/true/false/checked/unchecked/blank/empty` 及 `_` 前缀变体。
**三态语义**(默认 `bools_are_tristate = true`,`db/search.py:399`):`field:true` 匹配"值为真 **或** 未定义";`field:false` **只**匹配"未定义"。这与直觉不同,必须原样实现。
**空值**:`field:`(空)匹配不到任何东西 —— 与 calibre 一致(`db/search.py:599`)。"有该字段"要写 `field:true`。
**日期**(`db/search.py:164-183`):
- 运算符 `> >= < <= = !=`。
- 特殊值 `today` / `yesterday` / `thismonth` / `NN_daysago`。
- **日期也接受 `true` / `false`** = "有日期" / "无日期"。`pubdate:true` 是很常用的查询,不能漏。
- 比较精度由值中 `-` 或 `/` 的数量决定:`pubdate:>2020` 按年比,`pubdate:>2020/01` 按月比,`pubdate:>2020-01-01` 按日比。
- `date` 是 `timestamp` 的搜索别名。
**数值/评分**(`db/search.py:305-312, 685`):
- 运算符 `= != < > <= >=`,支持 `k/m/g` 后缀。
- 评分内部 0–10(2×星)。比较时查询值**向下取整**(`x // 2` 转成星),`rating:>=3` ⇒ 存储值 ≥ 6;`rating:>=3.5` ⇒ `int(3.5) = 3` ⇒ 存储值 ≥ 6。
- 开启半星(`allow_half_stars`)时字段变为 half-rating,查询值按 `round(q * 2)` 处理。
**标识符**:`identifiers:test:one`;`isbn:` 重写为 `=isbn:<query>`;`identifiers:=test:=two` 表示"键和值都精确匹配"。
**计数**:`tags:#<2`(少于 2 个)、`tags:#>2`、`formats:#=1`。
**自由文本**:`all:`(裸词默认落这里)搜所有带搜索词的字段。`all:` 路径有自己的 `'true'/'false'` 特例(匹配"该字段有任意非空值"),**与逐字段的布尔语义不同**(`db/search.py:869-876`)。
**特殊位置**:`all`、`search:<名称>`(保存的搜索,可递归,需防环)、`vl:<名称>`(虚拟库)、`template:<模板>#@#:<t|n|d|b>:<查询>`、`@UserCategory:item`。

**明确不实现**:calibre 的元数据语言里**没有** `in`/`not in`、没有范围运算符(仅 FTS 引擎与日期比较有)—— 不要自创。

**实现方式**:解析为 AST → 编译为 SQLite `WHERE` 子句(比 calibre 的 Python 集合运算快得多),`all:` 展开为跨字段 `OR`。`search:` / `vl:` 递归展开时做环检测。FTS5 作为独立的 `fts_search(query)` 接口,不复用元数据语言。

**对拍要求**:用 Python 版 calibre 与本实现对同一书库跑同一批查询,结果书 ID 集合必须完全一致(见 §7)。对拍标准是**逐位一致(含上游怪癖)**,实现者不得"顺手修正"上游行为。

### D4. 字段系统

**决策**:完整对齐 calibre 的 `library/field_metadata.py` + `db/fields.py`。

**内置字段**(不加前缀):

| key | 语义 | 存储 |
|---|---|---|
| `title` | 书名 | `books.title` |
| `sort` | 书名排序键(= `title_sort(title)`) | `books.title_sort` |
| `authors` | 作者(多值) | `authors` + `books_authors_link` |
| `author_sort` | 作者排序串 | `books.author_sort` |
| `tags` | 标签(多值) | `tags` + `books_tags_link` |
| `series` | 系列(多对一) | `series` + `books_series_link` |
| `series_index` | 系列序号(float) | `books.series_index` |
| `series_sort` | 系列排序键(虚拟) | `title_sort(series)` |
| `publisher` | 出版商(多对一) | `publishers` + link |
| `rating` | 评分 0–10(2×星) | `ratings` + link |
| `languages` | 语言(多值) | `languages` + link |
| `formats` | 已有格式(多值,虚拟) | `data` 表派生 |
| `identifiers` | 标识符键值对 | `identifiers` |
| `comments` | 评论(HTML) | `comments` |
| `cover` | 是否有封面(虚拟 bool) | `books.has_cover` |
| `id` | 书 ID | `books.id` |
| `pages` | 页数 | `books_pages_link` |
| `pubdate` | 出版日期 | `books.pubdate` |
| `timestamp` | 导入日期(搜索词 `date`) | `books.timestamp` |
| `last_modified` | 最后修改时间 | `books.last_modified` |
| `uuid` | 全局唯一 ID | `books.uuid` |
| `size` | 文件总大小 | `books.size` |
| `path` | 相对路径 | `books.path` |

**自定义列**:`#` 前缀(`#mycol`)。datatype:`rating / text / comments / datetime / int / float / bool / series / composite / enumeration`。`is_multiple` 仅对 `text` 与 `composite` 有效。归一化类型(text/series/rating/enumeration)用"值表 + link 表";非归一化(datetime/comments/int/bool/float/composite)用"每书一行"表。`series` 类型额外在 link 行存 `extra` 作为序号。

**复合列(composite)**:用 calibre 的模板语言求值(见 D9,模板系统是独立工作项,列入 Phase 4)。

**排序键**:
- 文本/系列/枚举:ICU `sort_key`(collator),`is_multiple` 且 `display.sort_alpha` 时按值的排序键元组排。
- 数值/评分:`numeric_sort_key`,未定义值由偏好 `value_for_undefined_numbers_when_sorting` 决定。
- 布尔:三态下 `true=1, false=2, none=3`。
- 日期:清洗后的日期。
- `authors` 用 `author_to_author_sort`。
- 排序字段名映射:`{'title':'sort', 'authors':'author_sort'}`;系列类字段按 `(series_sort_key, series_index)` 排。
- **默认排序:按导入时间 `timestamp` 降序(最新导入的排在最前)。**
  上游 `gui2/library/__init__.py:4` 是 `DEFAULT_SORT = ('timestamp', False)`,而元组第二项的含义是 **ascending 标志**(`db/cache.py:1768-1776` 的 docstring 明确写 "a list of 2-tuples of the form (field_name, ascending=True or False)";GUI 侧 `models.py:1906` 写 `self.sorted_on = (col, not descending)`)。因此 `False` = **非升序 = 降序 = 最新在前**。
  ⚠️ 这里的布尔方向极易读反,实现时以 `db/cache.py:multisort` 的 docstring 与 `models.py:_sort` 为准,不要凭直觉。

**title_sort 算法**(严格对齐 `ebooks/metadata/__init__.py:203`):
1. `order == "strictly_alphabetic"` → 原样返回。
2. 剥掉首尾成对引号(`"..."` / `“...”` / `«...»` / `„...„` / `‹...›`)。
3. 按语言取冠词表(`per_language_title_sort_articles`):英语 `A|The|An`;法语 `Le|La|L'/L´/L’/Les|Un|Une|Des|De (La)?/D'/D´/D’`;西语 `El/La/Lo/Los/Las/Un/Una/Unos/Unas`;意大利语 `Lo/Il/L'/L´/La/Gli/I/Le/Uno/Un/Una/Un'/Un´/Dei/Degli/Delle/Del/Della/Dello/Dell'/Dell´`;世界语 `La/L'/L´`;波兰语 `()`。
4. 命中冠词 → 把冠词移到末尾并加 `, `:`The Lord of the Rings` → `Lord of the Rings, The`。
5. 语言取值:书籍首个语言 → 偏好 `default_language_for_title_sort` → UI 语言 → 回退英语表。

**author_to_author_sort 算法**(严格对齐 `ebooks/metadata/__init__.py:67`):
1. `method = "copy"` → 原样。`method = "comma"` 且名字已含逗号 → 原样(所以 `Last, First` **永远不被重排**)。
2. 剥掉方括号内容(**逗号检查在剥括号之后做** —— `metadata/__init__.py:84-86`)。
3. 任一 token 命中 `author_name_copywords`(Agency/Corporation/Company/Co./Council/…) → 原样。
4. 跳过前缀 `author_name_prefixes`(Mr/Mrs/Ms/Dr/Prof/…)与后缀 `author_name_suffixes`(Jr/Sr/Inc/Ph.D/MD/I/II/III/IV/Junior/Senior/…)。
5. 姓氏前缀(`da/de/di/la/le/van/von`)是否参与由偏好 `author_use_surname_prefixes` 决定(默认 **否**);**且该规则仅对恰好两个 token 的名字生效**(`metadata/__init__.py:98-103`)。
6. 结果 `Surname, Given … Suffix`;`method = "nocomma"` 时输出 `Surname Given`。

**作者拆分**:`string_to_authors` —— `&&` 是转义;分隔符默认正则 `(?i),?\s+(and|with)\s+`(偏好 `authors_split_regex`),可被改写为 `&`;按**任意单个** `&` 切分。`"Author & Co"` → `["Author","Co"]`;`"Smith && Sons"` → `["Smith & Sons"]`。存储时 `,` 替换为 `|`(calibre 的兼容做法),排序时还原。

### D5. 主界面结构

**决策**:严格对齐 calibre 5+ 的主窗口。

**顶层结构**(自上而下):
1. **工具栏**(calibre 5+ 是 toolbar-driven)。**macOS 的默认布局**(`gui2/__init__.py:359-376`,平台条件分支):
   ```
   Add Books | Edit Metadata | ‖ Convert Books | View | ‖ Choose Library | Donate | ‖
   Fetch News | Store | Save To Disk | Connect Share | ‖ Remove Books | Tweak ePub
   ```
   注意:macOS 默认工具栏上**没有 Help、没有 Preferences** —— 它们在原生菜单栏里。
   (非 macOS 的默认布局是 `gui2/__init__.py:399-417`,含 Help/Preferences、且 Choose Library 位置不同。本项目只做 macOS,故采用前者。)
   顺序与可见性由偏好 `action-layout-toolbar` 控制(可拖拽定制)。
2. **macOS 原生菜单栏**:`Add Books / Edit Metadata / Convert Books / Choose Library / Save To Disk / Preferences / Help`。
   非 macOS 上菜单栏为空(全部在工具栏)—— 一期只做 macOS,但保留该结构。
3. **搜索栏行**:
   `[虚拟库按钮] [清除VL] [排序▾] [分组▾] [搜索框] [FTS图标·前置] [齿轮图标·前置] [解析错误图标·后置] [搜索] [高亮]`
4. **中央容器**:5 面板 + 4 分隔条(`left_handle` / `right_handle` / `top_handle` / `bottom_handle`)。
   - 宽布局:左=标签浏览器,中=书列表,右=书籍详情,上=封面浏览器,下=快速预览。
   - 窄布局(Alt+Shift+L 切换):左=标签浏览器(0.25 宽),右=封面浏览器(0.35 宽),上=快速预览(0.2 高),下=书籍详情(0.3 高),中=书列表。
   - 拖分隔条低于 10px 隐藏对应面板,超过 50px 恢复显示。
   - 默认可见:标签浏览器 ✓、书籍详情 ✓、书列表 ✓;封面浏览器 ✗、快速预览 ✗。
5. **状态栏**:显示书籍数量。

**标签浏览器(左)**:
- 树结构,节点类型 `CATEGORY` / `TAG` / `ROOT`。
- 类别集合:字段中 `is_category` 为真的 —— `authors / languages / series / formats / publisher / rating / news / tags / identifiers` + 自定义列(归一化的)+ 用户类别(`@` 前缀)+ 保存的搜索(`search` 类别)。
- 显示顺序:标准字段 → 自定义列(字母序)→ 用户类别 → 保存的搜索。
- 排序模式:名称 / 书数 / 平均评分。
- 每个类别可显示条目数;`rating` 类别显示平均星级(原始分 ÷2)。
- 支持层级类别(名字中的 `.`)、按首字母分区。
- 交互:点击过滤、右键菜单(重命名 / 删除 / 编辑作者 / 打开管理器 / 设为用户类别 / 添加子类别 / 隐藏类别 / 搜索 / 折叠 / 移除格式等)、把书拖到类别上打标签。
- **"查找"框是跳转(找下一个),不是实时过滤**(`gui2/tag_browser/ui.py:781-806`):输入后按"查找"按钮在树里定位下一个匹配项,并显示 *"No more matches. Click Find again to go to first match"* 之类的提示。**不是**输入即隐藏不匹配的行 —— 实现时不要做错交互模型。
- 另有一个"只显示含有可见类别的书"的过滤开关(`ui.py:924`)。

**书列表(中)**:
- 列来自 `displayable_field_keys()`;默认隐藏 `last_modified / languages / formats / id / path / pages`。
- 默认排序:按导入时间**降序**(最新在前,见 D4);最多 3 级排序(点击列头切换,`tweaks['maximum_resort_levels']`)。
- 不可排序列:排序键、作者排序串、`au_map`、`series_sort`、`marked`、`series_index`、`path`、`formats`、`identifiers`、`uuid`、`comments`。
- 列头:拖动重排、右键菜单(按 X 排序 / 对齐 / 字体样式 / 隐藏显示 / 调整列宽 / 固定列)。
- 三种视图共享同一数据模型:**列表视图**(表格)、**封面网格视图**(Alt+Shift+G)、**3D 书架视图**(Alt+Shift+H)。
- 多选、拖出到外部应用、从分类拖入。

**书籍详情(右)**:
- 内部是 splitter:左侧 `CoverView`(封面),右侧 `BookInfo`(元数据表)。
- 元数据表按偏好 `book_display_fields` 的字段顺序渲染,值可点击(跳转到搜索/打开 URL/打开格式),支持多列排布。
- 评论按 `comments_pat` 清洗后以 HTML 渲染。
- "编辑元数据"是独立模态对话框(不是内联编辑)。

**搜索**:
- 搜索框 1500ms 防抖,历史 25 条(持久化到 `main_search_history`)。
- 下拉是**历史**而非"搜索模式"(calibre 5+ 没有 all/title/author 模式切换)。
- 字段定向靠搜索语法或**高级搜索**对话框。
- "附加限制"菜单:无 / `*当前搜索` / 文本限制 / 已保存的搜索。
- **虚拟库**:具名搜索式,叠加在当前搜索之上;清除按钮上显示 `基础VL :: 限制`(截断到 20 字符)。
- 状态栏显示当前显示的书数。

**GPUI 映射**:

| calibre 构件 | GPUI 实现 |
|---|---|
| 书列表表格 | `ui::Table`(`crates/ui/src/components/data_table.rs:356`):列宽可调、列筛选、`uniform_list` 虚拟滚动、斑马纹 |
| 标签浏览器树 | 自实现:`uniform_list` + 缩进层级 + 展开状态 + 键盘上下移动/左右折叠。行渲染可复用 `ui::TreeViewItem` |
| 输入框 | 参考 `crates/gpui/examples/input.rs` 自实现 `InputField`(`EntityInputHandler` + `FocusHandle` + `ShapedLine` + 选区 + 剪贴板)。**明确不用** `ui_input::InputField`:它要求 `ERASED_EDITOR_FACTORY`,而唯一 setter 在 `editor` crate,等于拖入 Zed 整个编辑器栈 |
| 主题/字体 provider | **自实现** `ThemeSettingsProvider`(5 个方法:`ui_font` / `buffer_font` / `ui_font_size` / `buffer_font_size` / `ui_density`)。不引入 zed 的 `theme_settings` crate(它拖入 `settings` → `fs` → `notify`,并需再补两个 patch)。启动时必须 `theme::init(theme::LoadThemes::JustBase, cx)`,否则 `ui` 组件 panic |
| 元数据表(HTML) | 自实现受限 HTML → `Vec<TextRun>` 渲染器(覆盖 `b/i/span/a/h1-h6/br/ul/ol/li/img/table`),不引入 Zed 的 `markdown`/`html_to_markdown` crate(API 面向 Zed editor,易漂移) |
| 封面 | `gpui::img()` + `ImageSource`,`image` crate 解码 |
| 菜单 / 右键菜单 | 原生菜单 `cx.set_menus([...])`;弹出菜单用 `ui::ContextMenu` / `ui::Popover` |
| 模态对话框 | `ui::Modal`;系统级确认用 `window.prompt(PromptLevel::...)` |
| 文件选择 | `cx.prompt_for_paths(PathPromptOptions { ... })` |
| 拖拽 | `.on_drag()` / `.on_drop::<T>()` |
| 快捷键 | `gpui::actions!` + `KeyBinding::new` + `cx.bind_keys` |
| 3D 书架 | GPUI canvas 自绘(参考 `crates/gpui/examples/painting.rs`),Phase 2 |

**i18n**:
- `calibre-i18n` crate,消息目录用 **Fluent**(`.ftl`):支持复数规则与参数,运行时可切换 locale,生态成熟。
  - 若评审认为 Fluent 过重,退路:`rust-i18n`(JSON 目录)。**本项目选 Fluent**。
- 全部 UI 文案走 `t!("key")` / `t!("key", {"name" => name})`;代码里不出现硬编码英文字面量。
- 语言切换:全局 locale 存于 `Rc<Cell<Locale>>`,切换后触发主窗口重绘。

### D6. 格式与元数据层

**决策**:完整对齐 calibre 的三段式插件架构,中间表示为 OEBBook。

**元数据模型**(`Metadata` 归 **`calibre-core`**,见 §4.1 的依赖方向约束):
- 字段分组:SOCIAL(`tags` / `rating` / `comments` / `series` / `series_index` / `identifiers`)、PUBLICATION(`title` / `title_sort` / `authors` / `author_sort_map` / `author_sort` / `book_producer` / `timestamp` / `pubdate` / `last_modified` / `rights` / `publication_type` / `uuid` / `languages` / `publisher` / `cover` / `cover_data` / `thumbnail`)、STRUCTURE(`toc` / `spine` / `guide` / `manifest`)、USER(`user_metadata`)、CALIBRE(`formats` / `user_categories` / `link_maps` / `pages`)。
- `cover_data` 恒为 `(format, bytes)`;`cover` 为绝对路径。
- 标识符键小写并去掉 `:`/`,`;值里的 `,` 替换为 `|`。

**插件 trait**(Rust 化):

```rust
pub trait InputFormatPlugin: Send + Sync {
    fn file_types(&self) -> &[&str];
    fn convert(&self, stream: &mut dyn Read, opts: &ConversionOptions) -> Result<OEBBook>;
    fn postprocess_book(&self, _book: &mut OEBBook) -> Result<()> { Ok(()) }
    fn specialize(&self, _book: &mut OEBBook) -> Result<()> { Ok(()) }
}
pub trait OutputFormatPlugin: Send + Sync {
    fn file_type(&self) -> &str;
    fn convert(&self, book: &OEBBook, out: &mut dyn Write, opts: &ConversionOptions) -> Result<()>;
}
pub trait MetadataReaderPlugin: Send + Sync {
    fn file_types(&self) -> &[&str];
    fn get_metadata(&self, stream: &mut dyn Read) -> Result<Metadata>;
}
pub trait MetadataWriterPlugin: Send + Sync {
    fn file_types(&self) -> &[&str];
    fn set_metadata(&self, stream: &mut dyn ReadWrite, meta: &Metadata) -> Result<()>;
}
```

**EPUB 是优先支持格式(用户指定:「EPUB2 和 EPUB3 为优先支持格式」)**

EPUB2 与 EPUB3 是**第一批就要做到位**的格式,排在所有其他格式之前。EPUB 的读写、元数据、封面、目录、阅读器支持,必须完整且正确。

**两个版本的技术差异**(必须分别正确处理,不能只做其中之一):

| 维度 | EPUB 2(OPF 2.0) | EPUB 3(OPF 3.0) |
|---|---|---|
| package | `version="2.0"` | `version="3.0"` |
| 目录 | NCX(`toc.ncx` + `spine toc="ncx"`)+ `guide` | 导航文档 `nav.xhtml`(`epub:type="toc"`),manifest 项 `properties="nav"`;landmarks 导航 |
| 必需元数据 | 无额外要求 | **必须**有 `<meta property="dcterms:modified">`(UTC 时间戳) |
| 封面 | `<meta name="cover" content="<manifest-id>"/>` | manifest 项 `properties="cover-image"` |
| manifest `properties` | 不可用 | 可用 `svg` / `scripted` / `mathml` / `switch` |
| XHTML | XHTML 1.1 | HTML5 + `epub:type` 允许 |

**实现路径**:与上游一致 —— **先统一按 EPUB2 结构生成**,再用 `epub_2_to_3`(`src/calibre/ebooks/oeb/polish/upgrade.py:152`)升级:
1. `collect_properties`(`upgrade.py:67`)—— 扫描各 manifest 项内容,自动打上 `svg` / `scripted` / `mathml` / `switch` 标签。
2. `create_nav`(`upgrade.py:140`)—— 由 NCX 的 TOC 生成 `nav.xhtml`,并写入 landmarks;`commit_nav_toc` 负责落地。
3. 移除 NCX(`remove_ncx=True` 为默认)。
4. 补 `dcterms:modified`。

上游的 `epub_output.py` 也是这个流程:`epub_version` 选项(choices `('2','3')`,**默认 `'2'`**,`epub_output.py:76,144-149`),值为 `'3'` 时在 `epub_output.py:300` 调用 `upgrade_to_epub3`(`epub_output.py:355-374`)。
**默认输出 EPUB2**(上游注释:*"EPUB 2 is the most widely compatible"*),EPUB3 需显式指定。

**读取端**:两个版本都要能读。EPUB3 的 `nav.xhtml` 与 EPUB2 的 NCX 归一到同一个内部 TOC 结构;`dcterms:modified` 与 `dc:date` 归一到同一个 `pubdate`/`timestamp`。

**元数据边车**:本项目自己的 `metadata.json` 与 EPUB 版本无关;写回 EPUB 时按目标版本降级/升级(§D6 的 `MetadataWriterPlugin` 负责)。

**格式支持矩阵**(对齐上游 `src/calibre/ebooks/conversion/plugins/` 的实际插件集合):

> **上游插件目录已重构**:calibre 9.x 把所有转换插件集中到 `src/calibre/ebooks/conversion/plugins/{format}_{input,output}.py`。`ebooks/epub/`、`ebooks/mobi/`、`ebooks/fb2/` 等目录下**已无插件代码**(只剩 `cfi/`、`pages.py` 等辅助模块)。查上游实现时不要去 `ebooks/<format>/` 找插件。
>
> **AZW / AZW3 没有独立插件**:AZW3 由 `MOBIInput` 读、`AZW3Output` 写(在 `mobi_output.py` 内);**AZW 上游根本没有插件**。实现 MOBI 一次即可顺带获得 AZW3。

| 格式 | 上游插件 | 读元数据 | 写元数据 | 转换源 | 转换目标 | 阶段 |
|---|---|---|---|---|---|---|
| **EPUB 2** | `epub_input` / `epub_output` | ✅ **P0** | ✅ **P0** | ✅ **P0** | ✅ **P0** | Phase 1(读)/ Phase 2(写) |
| **EPUB 3** | 同上(`epub_version='3'`) | ✅ **P0** | ✅ **P0** | ✅ **P0** | ✅ **P0** | Phase 1(读)/ Phase 2(写) |
| KEPUB | `epub_output`(变体) | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| TXT / TXTZ | `txt_input` / `txt_output` | ✅ | ❌ | ✅ | ✅ | Phase 2 |
| HTML / XHTML | `html_input` / `html_output` | ✅ | ❌ | ✅ | ✅(产出 zip) | Phase 2 |
| HTMLZ | `htmlz_input` / `htmlz_output` | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| FB2 / FBZ | `fb2_input` / `fb2_output` | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| MOBI(含 AZW3) | `mobi_input` / `mobi_output` | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| RTF | `rtf_input` / `rtf_output` | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| ODT | `odt_input` | ✅ | ❌ | ✅ | ❌ | Phase 2 |
| ~~PDF~~ | `pdf_input` / `pdf_output` | 占位 | 占位 | 占位 | 占位 | **暂不做**(见 §8 决策 27) |
| Comic(CBZ/CBR/CB7/CBC) | `comic_input` | 部分 | ❌ | ✅ | ❌ | Phase 3 |
| LRF | `lrf_input` / `lrf_output` | ✅ | ✅ | ✅ | ✅ | Phase 3 |
| PDB(eReader) | `pdb_input` / `pdb_output` | ✅ | 部分 | ✅ | ✅ | Phase 3 |
| SNB | `snb_input` / `snb_output` | ✅ | ❌ | ✅ | ✅ | Phase 3 |
| TCR | `tcr_input` / `tcr_output` | ❌ | ❌ | ✅ | ✅ | Phase 3 |
| PML | `pml_input` / `pml_output` | ✅ | ❌ | ✅ | ✅ | Phase 3 |
| RB(RocketBook) | `rb_input` / `rb_output` | ✅ | ❌ | ✅ | ✅ | Phase 3 |
| DJVU | `djvu_input` | ❌ | ❌ | ✅ | ❌ | Phase 3 |
| CHM | `chm_input` | ✅ | ❌ | ✅ | ❌ | Phase 3 |
| LIT | `lit_input` / `lit_output` | ✅ | ❌ | ✅ | ✅ | Phase 3 |
| DOCX | `docx_input` / `docx_output` | ✅ | ❌ | ✅ | ✅ | Phase 3 |
| AZW4 | `azw4_input` | ✅ | ❌ | ✅ | ❌ | Phase 3 |
| 压缩包 | `archive_input` | ❌ | ❌ | ✅ | ❌ | Phase 3 |
| OEB(中间产物) | `oeb_output` | ❌ | ❌ | — | ✅ | Phase 3 |
| Recipe(新闻) | `recipe_input` | ❌ | ❌ | ✅ | ❌ | 不做(见 §6 Phase 6) |
| KFX | — | ❌ | ❌ | ❌ | ❌ | 不支持(DRM) |
| ZEBRA | — | ❌ | ❌ | ❌ | ❌ | 上游亦不支持 |

**转换管线**(对齐 `ebooks/conversion/plumber.py`):输入插件 → OEB 变换链(`DataURL` → `Clean(guide)` → `RemoveFirstImage` → `MergeMetadata` → `DetectStructure` → **`CSSFlattener`** → `Jacket` → `AddAltText` → `LinearizeTables` → `UnsmartenPunctuation` → `RemoveFakeMargins` → `RemoveAdobeMargins` → `EmbedFonts` → `SubsetFonts` → `ManifestTrimmer`)→ 输出插件。
> 注意 `CSSFlattener` 紧跟在 `DetectStructure` 之后(`plumber.py:1332`),不是排在 `UnsmartenPunctuation` 之后。上游整个管线是 `Plumber.run()` 这一个约 270 行、带内联阶段注释的方法(`plumber.py:1188-1460`),不是一串变换对象 —— 实现时可拆成对象,但**阶段顺序必须一致**。

- 选项模型:calibre 的 `OptionRecommendation`(`name` / `help` / `long_switch` / `short_switch` / `choices` / `recommended_value` / `level`),合并顺序 `input_options + pipeline_options + output_options + all_format_options`。
- **选项总量:84 个 pipeline 选项 + 162 个格式专属选项 = 246**。`ebook-convert` 的 84 个 pipeline 选项分批实现(Phase 2 先做高频 30 个,Phase 3 补齐)。

**封面**:
- 库内封面存 `<书目录>/cover.jpg`;`books.has_cover` 标记。
- 默认封面生成:按 calibre 的 `create_cover` 规则(标题/作者/系列三行版式 + 随机配色主题)用 GPUI 渲染或用 `image` crate 直接绘制。
- 嵌入:EPUB2 `<meta name="cover">` / EPUB3 `properties="cover-image"`;MOBI EXTH 记录 201/202/203。

**元数据边车**:`metadata.json`(本项目自定,替代 OPF2)。

### D7. 阅读器(纯 Rust 文本渲染)

**决策**:第一期用纯 Rust 文本渲染,定义 `ReaderRenderer` trait 预留 WebView 实现位置。

- 链路:EPUB(XHTML) / FB2 / TXT → 内容抽取 → `Vec<Block>`(段落 / 标题 / 图片 / 列表 / 分隔符)→ 样式化 → `Vec<TextRun>` → GPUI 文本布局分页渲染。
- HTML 处理:用 `html5ever` 解析;CSS 只支持内联 `style` 与 `<style>` 块中的 `font-family / font-size / font-weight / font-style / color / text-align / text-indent / text-decoration / margin-* / line-height`。
- 图片:抽取后按可用宽度缩放内联渲染。
- 阅读器功能(对齐 calibre):目录导航、字体/字号/行距/页边距、主题(日间/sepia/夜间)、滚动模式与分页模式、书内搜索、书签、注释/高亮/下划线/删除、阅读位置同步(存 `last_read_positions`)。
- **明确的能力差距**:CSS 布局(多列、float、绝对定位)不支持;JS 不支持(calibre 的阅读器模板也基本不用 JS)。分页靠文本测量 + 断行近似,不保证与 calibre 逐像素一致。

### D8. 插件系统

**决策**:定义 Rust trait 插件接口,**第一期编译期注册**(内置插件),不引入动态库加载。

- 对应 calibre 的插件类型:`InputFormatPlugin`、`OutputFormatPlugin`、`MetadataReaderPlugin`、`MetadataWriterPlugin`、`CatalogPlugin`、`InterfaceAction`(界面动作)、`PreferencesPlugin`、`StorePlugin`、`EditBookToolPlugin`、`ContentServerPlugin`。
- 动态加载(cdylib + `dlopen`)列为 Phase 6 的可选项,前提是 trait 全部对象安全且用 `extern "C"` ABI 描述。
- **不实现 Python 插件兼容**。这是有意的取舍:calibre 生态里大量第三方插件基于 Python,放弃它们是"另起炉灶"的必然结果。

### D9. 模板系统(单点风险,单独排期)

calibre 的模板语言(General Program Mode + 模板表达式)是复合列、`save_template`、`metadata_source` 的基础,规模约 3000 行 Python。

- **决策**:Phase 4 实现一个**子集**:字段引用(`{title}`)、条件表达式(`{if:cond,then,else}`)、通用程序模式(`{python:...}` 的受限版:变量、比较、算术、`if/for`、`=`,不含 import/eval)、字符串函数(`title_sort`、`author_to_author_sort`、`fmt_sidx`、`date_string`…)。
- **风险**:完整对齐 calibre 的模板语言是本项目最大的单点工作量。**不追求 100% 兼容**,但要能覆盖复合列的常见用法。
- **缓解**:模板系统独立成 `calibre-core::template` 模块,可单测;Phase 1/2 不依赖它(复合列在 Phase 1 先以"仅支持 text/整数"方式存在,Phase 4 再接模板)。

### D10. 内容服务器与 CLI

- `calibre-server` 用 `axum`:`/` 书架浏览、`/book/<id>` 详情、`/book/<id>/<format>` 下载、`/opds` OPDS 1.2/2.0 目录、`/search`、封面与静态资源。鉴权沿用 calibre 的"http 基础认证 + 局域网放行"模型。
- CLI(Phase 5):
  - `calibredb` 等价:`list` / `add` / `remove` / `export` / `import` / `backup` / `restore` / `check` / `catalog` / `custom_columns` / `set_metadata` / `show_metadata` / `search` / `saved_searches` / `clone` / `embed_metadata` / `fts_index` / `fts_search`。
  - `calibre-debug` 等价:运行库完整性检查、schema 打印、调试工具。
  - `ebook-convert` 等价:转换。
  - `ebook-edit` 等价:编辑。

### D11. 从既有 calibre 书库导入

**决策**:一期必须支持(用户明确要求)。

- 读取 `metadata.db`(**只读**打开,绝不写回)解析:books / authors / tags / series / publishers / ratings / comments / identifiers / languages / data / link 表 / custom_columns。
- 扫描 `作者/书名 (id)/` 目录,定位格式文件与 `cover.jpg`。
- `metadata.opf` 作为元数据的**兜底来源**(当 db 缺字段时)。
- 转换映射:
  - `authors` 名字的 `|` 还原为 `,`。
  - `pubdate` / `timestamp` 的 calibre 格式解析(支持 `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, `YYYY-MM-DD HH:MM:SS`, `MM/DD/YYYY` 等)→ ISO8601。
  - `rating` 已是 0–10,直接搬。
  - `series_index` float 直接搬。
  - `identifiers` 键小写、值 `|` 还原为 `,`。
- 导入后写入本项目的新库与目录;原 calibre 书库**只读不改**。

### D12. 美术资源与字体

**决策(用户已拍板)**:**图标直接使用 calibre 原版**(`../calibre/imgsrc/*.svg`,193 个);**不引入** zed 的 `assets` crate;字体自带 **Noto Sans SC**。

**图标来源与落地方式**:
- calibre 的图标源文件在 `calibre/imgsrc/`(193 个 SVG + `imgsrc/plugins/` 子目录),由 `calibre/imgsrc/generate.py` 在构建期用 `rsvg-convert` 栅格化成 PNG 打进 Qt 资源主题 —— **这一步我们不需要**。
- **GPUI 原生支持 SVG**(`crates/gpui/src/elements/img.rs:216` 把 `svg` 列入支持的扩展名,用 `resvg`/`usvg` 渲染,`cx.svg_renderer()`)。因此**直接打包 SVG 即可**,不必栅格化。
- **落地方式**:把 `imgsrc/*.svg` **复制进本仓库的 `assets/icons/`**(不构建期引用兄弟目录,保持仓库自包含),并在 `assets/NOTICE` 里记录来源(calibre, GPL-3.0-or-later)与原始文件列表。
- **主题变体选择规则**:calibre 有按主题区分的图标(`close-for-dark-theme.svg` / `close-for-light-theme.svg`、`external-link-for-dark-theme.svg`、`notes-for-dark-theme.svg`,以及自动反色的 `modified-for-dark-theme.svg` / `modified-for-light-theme.svg`)。解析规则:**给定图标名 N,先找 `N-for-{当前主题}-theme.svg`,找不到则用 `N.svg`**。深色主题下,若只有 `N.svg` 而存在 `modified-for-dark-theme.svg`,用后者。
- `calibreSymbols.spd`(Spline 符号字体)我们**不使用** —— 直接用单个 SVG,更可控。

**字体**:
- **必须自带中文字体**:zed 的 `assets` 只有拉丁字体,中文会渲染成豆腐块。字体选 **Noto Sans SC**(SIL OFL 1.1,可自由分发与嵌入)。
- 拉丁字形建议同用 Noto Sans 系列,保持视觉一致。
- 在启动时通过自建的 `AssetSource` + `gpui` 的字体注册 API 加载。

**AssetSource 实现**:
- 实现 `gpui::AssetSource`(`crates/gpui/src/assets.rs:13`),提供 `assets/icons/*.svg` 与 `assets/fonts/*`。
- **不使用** `ui::Icon` 搭配 Zed 的 `IconName` 枚举(那些 SVG 在 `assets` 里,我们没有);改为自建图标名空间,名字沿用 calibre 的语义名(`book.svg` / `books_in_series.svg` / `convert.svg` / `search.svg` / `gear.svg` / `metadata.svg` / `remove_books.svg` / `save.svg` / `tags.svg` / `rating.svg` / `reader.svg` / `trash.svg` / `sort.svg` / `layout.svg` / `cover_flow.svg` / `bookshelf.svg` / `fts.svg` / `vl.svg` / `notes.svg` / `toc.svg` / `markers.svg` …),与上游一一对应,方便对照实现。

**取舍**:图标与 zed 的 `ui`/`theme`/`icons` 同为 GPL-3.0-or-later;字体用 OFL 1.1。整体仓库按 GPL-3.0-or-later 分发(见 §8 决策 26)。

---

## 6. 分阶段路线图

每个阶段的"完成"= 该阶段列出的验收项全部通过,且对应测试通过。

### Phase 0 —— 依赖可行性验证(spike)

1. `RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1`。
2. 建 workspace 骨架 + `Cargo.toml`(含 §D1 的 patch 与 `runtime_shaders`)+ `rust-toolchain.toml` + `.cargo/config.toml`。
3. 最小 `main.rs`:`gpui_platform::application().run(...)` 开窗,内含:
   - `theme::init(theme::LoadThemes::JustBase, cx)`
   - 自实现的 `ThemeSettingsProvider` 注册
   - 自建 `AssetSource` + 中文字体注册
   - 一个 `ui::Button` + 一段中文文本
4. `cargo build --target aarch64-apple-darwin` 通过(避免 `cargo metadata` 拖垮网络)。

**验收**:窗口打开、中文文本正常显示(不是豆腐块)、按钮可点。
**已验证部分**:2026-10-21 在临时目录用上述 manifest 完整编译并运行了 GPUI 栈 —— 编译通过、进程存活、无 Metal/shader 错误。
**未验证部分**:**像素渲染未做肉眼确认**(本机 `screencapture`/`osascript` 被系统权限拒绝)。**必须在有录屏权限的机器上补这一步**。
**风险**:若 `async-task` patch、`gpui_platform` 特性或 `runtime_shaders` 不匹配,此处即暴露。

### Phase 1 —— 书库内核 + 主界面(用户选定的首个里程碑)

**范围调整(重要)**:Phase 1 的验收要求"导入的书要有正确的标题/作者/系列/日期"和"能编辑封面",这**必须**用到格式解析能力;因此把 D6 的**一小片**提前到 Phase 1,而不是把 Phase 1 的导入范围缩水。

1. `calibre-core`:schema 迁移、字段注册表、`Metadata` 模型、存储层、排序键。
2. `calibre-core`:搜索语言解析器 + SQL 编译器 + 单元测试(覆盖 D3 全部语法分支)。
3. `calibre-formats`(**最小切片**):**EPUB2 + EPUB3 只读**元数据插件 + OEBBook 最小骨架 + 默认封面生成器。EPUB 是 P0 格式,读端必须同时正确处理 NCX 目录与 `nav.xhtml` 目录。
4. `calibre-core`:导入器 —— 单文件 / 多选 / 文件夹 / **从既有 calibre 书库**(D11)。
5. `calibre-app`:主窗口布局(工具栏 / 菜单栏 / 搜索栏 / 5 面板 / 分隔条 / 状态栏)。
6. `calibre-app`:标签浏览器树;书列表 `ui::Table`;书籍详情;搜索框(防抖 + 历史 + 限制 + 虚拟库)。
7. `calibre-app`:`InputField` 组件;元数据编辑模态框(单本)。
8. `calibre-i18n`:Fluent 骨架,zh-CN + en。

**验收**:建空库 → 导入若干书(从文件 + 从旧 calibre 书库)→ 元数据与封面正确 → 左栏点标签过滤 → 搜索框过滤 → 右栏显示元数据 → 编辑标题/作者/标签/封面 → 重启后数据仍在。
**交付标准**:`calibre-core` 与 `calibre-formats` 单测覆盖搜索与排序;对拍脚本跑通 §7.1 的元数据/搜索/排序三项;手动验收清单逐条对照。

### Phase 2 —— 格式与元数据

1. `calibre-formats`:`Metadata` 序列化(`metadata.json` 边车)与 `Metadata` ↔ `OEBBook` 转换(元数据模型本体已在 Phase 1 落进 `calibre-core`)。
2. `calibre-formats`:**EPUB2 / EPUB3 写**(P0)—— 统一按 EPUB2 结构生成,再按需 `upgrade_to_epub3`;封面嵌入、EPUB3 的 `dcterms:modified` 与 `nav.xhtml` 生成。
3. 格式插件补齐:TXT / HTML / HTMLZ / FB2 / MOBI(含 AZW3)/ RTF / ODT。**PDF 留占位符**(§8 决策 27)。
4. 转换管线骨架 + 高频 30 个 pipeline 选项 + `ebook-convert` 等价 CLI。
5. `calibre-app`:转换为 GUI 动作(工具栏"Convert Books")。

**验收**:同一本书用 calibre 与 calibre2Rust 转换,输出 EPUB 的正文与元数据一致(人工比对 + 抽样自动比对)。

### Phase 3 —— 阅读器 + 扩展格式

1. 阅读器骨架:内容抽取 → 块流 → 排版 → 分页/滚动。
2. **EPUB2 / EPUB3**(P0,目录同时支持 NCX 与 `nav.xhtml`)/ FB2 / TXT 阅读;目录导航;主题与排版设置;阅读位置。
3. 注释/书签/高亮 + 存储。
4. 补齐格式:Comic、LRF、PDB、SNB、TCR、PML、RB、DJVU、CHM、LIT、DOCX、AZW4。**PDF 仍为占位符**(§8 决策 27)。
5. 补齐剩余 pipeline 选项。

### Phase 4 —— 编辑器与模板系统

1. `calibre-core::template`:模板语言子集(§D9)。
2. 复合列接入模板;`save_template` 语义。
3. `ebook-edit` 等价 CLI + GUI 编辑器(Polish 基础:改元数据/改 CSS/改封面/改 TOC/拆书/合书)。

### Phase 5 —— CLI 与内容服务器

1. `calibredb` 等价命令全集。
2. `calibre-server` + OPDS + 浏览器端界面。
3. `calibre-debug` 等价。

### Phase 6 —— 插件系统与周边

1. 插件 trait 完备化;动态库加载(可选)。
2. 设备同步(Kindle/Kobo 等)—— 上游 calibre 有完整实现;本项目仅在用户明确要求时做,默认**不做**。
3. News recipes(新闻抓取)—— 上游是 160+ 个 Python recipe;本项目仅实现"自定义 recipe 的通用抓取框架",不逐个移植,默认**不做**。
4. Polish 高级功能、拼写检查、TTS、书内词典。

---

## 7. 验证方法

### 7.1 与上游 calibre 对拍(核心手段)

用一个 **Python 对拍脚本**(`tools/parity/`),对同一书库同时调用上游 calibre 与本实现,比对:

1. **元数据**:每本书的归一化元数据 JSON(title / title_sort / authors / author_sort / tags / series / series_index / publisher / rating / comments / identifiers / languages / pubdate / timestamp)逐字段比对。
2. **搜索**:一批固定查询(覆盖 §D3 全部语法分支:`=` `\ ` `~` `^` `=.` `=..`、三引号、日期 true/false、评分整除、三态布尔、`@UserCategory`、`search:` / `vl:`)+ 随机生成的查询,比对返回的书 ID 集合。
3. **排序**:对每种可排序字段 + 多种排序方向,比对完整书序。**含默认排序**(`timestamp` 降序)。
4. **title_sort / author_to_author_sort**:对一批(含 `&`、`&&`、逗号名、多语言冠词、方括号、罗马数字后缀、两 token 姓氏前缀)的样本,比对输出字符串。

对拍脚本用上游 calibre 的 Python API(`calibre.db.Cache` / `calibre.db.search.Search` / `calibre.ebooks.metadata` 的函数)作为参考实现。
**门禁:必须逐位一致(含上游怪癖)**;若本项目有意偏离,需在 `docs/superpowers/specs/` 下的偏离清单里逐条登记。

### 7.2 Rust 侧测试

- `calibre-core`:每个模块单测;搜索解析器的表驱动测试;SQL 编译器的快照测试。
- `calibre-formats`:每种格式用固定 fixture 做往返测试(读 → 写 → 再读 → 比对)。
- `calibre-app`:GPUI 的 `VisualTestAppContext` 在 macOS 上需主线程,标记 `#[ignore]`,手动跑;其余逻辑抽到纯函数以便单测。

### 7.3 手工验收清单

每个阶段维护一份 checklist,逐条对照上游 calibre 的实际行为(在 `../calibre` 源码树或真实 calibre 上核对)。

### 7.4 构建与质量门禁

```bash
cargo build --target aarch64-apple-darwin     # 编译(勿用 cargo metadata,会解析所有 target)
cargo fetch --target aarch64-apple-darwin      # 只拉当前平台依赖
cargo test -p calibre-core                    # 核心逻辑快速迭代
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

遵循 zed 的 Rust 规范:不用 `unwrap()`(用 `?` 传播)、不静默丢弃错误(`let _ =` 禁止用于可失败操作)、不写"总结代码"的注释、完整拼写变量名、不建 `mod.rs`、新 crate 用 `[lib] path = "src/xxx.rs"`。

---

## 8. 留给实现者的决策点(已由我拍板的部分)

| # | 决策 | 拍板 | 理由 |
|---|---|---|---|
| 1 | GPUI 依赖方式 | git + 固定 rev `92447a17` | 用户明确要求;crates.io 版本 API 过时 |
| 2 | `async-task` patch | 必须在消费方根 Cargo.toml 重复声明 | cargo 忽略依赖内的 patch |
| 3 | 工具链 | `rust-toolchain.toml` pin `1.98.1` | zed 的要求;直连 rust-lang.org 超时,须走 USTC 镜像 |
| 4 | 平台 | 一期只做 macOS | 减少变量;核心层保持跨平台 |
| 5 | 数据层 | 新 schema + SQLite(rusqlite) | 用户已确认另起炉灶 |
| 6 | 边车文件 | `metadata.json`(非 OPF2) | Rust 序列化更自然;放弃与 calibre 的互操作 |
| 7 | 磁盘布局 | 沿用 `作者/书名 (id)/` | 便于存量迁移与用户熟悉 |
| 8 | 搜索实现 | AST → 编译成 SQL | 比 calibre 的 Python 集合运算快,语义对齐 |
| 9 | 搜索对拍标准 | **逐位一致,含上游怪癖** | 语义含糊比不支持更糟;实现者不得"顺手修正"上游 |
| 10 | i18n 方案 | Fluent | 复数/参数/运行时切换成熟 |
| 11 | 排序/大小写 | ICU(`icu_collator` / `icu_casemap`) | 与 calibre 的 ICU 语义一致 |
| 12 | 阅读器 | 纯 Rust 文本渲染 + `ReaderRenderer` trait 预留 | 用户选择;避免 WebView 依赖 |
| 13 | 详情面板 HTML | 自实现受限 HTML → `TextRun` 渲染器 | 避免依赖 Zed 的 markdown crate(API 面向 editor,易漂移) |
| 14 | 插件 | 编译期注册优先,动态库加载后置 | 控制复杂度 |
| 15 | Python 插件兼容 | 不做 | 有意取舍 |
| 16 | 设备同步 / News recipes | 默认不做 | 上游体量大且强依赖 Python 生态 |
| 17 | crate 划分 | 4 个 crate(core / formats / i18n / app)+ server | GPUI 重编成本高,核心层要能独立快测 |
| 18 | `Metadata` 归属 | 归 `calibre-core`;`OEBBook` 归 `calibre-formats` | 避免 core ↔ formats 循环依赖 |
| 19 | Metal shader | 启用 `gpui_platform/runtime_shaders` | 本机无 Metal Toolchain;运行时编译可绕开 `xcrun metal` |
| 20 | 主题 provider | 自实现 `ThemeSettingsProvider`(5 方法) | 避免引入 `theme_settings` → `settings` → `fs` → `notify` 及额外 patch |
| 21 | 美术资源 | 自建 `AssetSource` + 自带 OFL 中文字体;不用 zed 的 `assets` | `assets` 无中文字体,中文会渲染成豆腐块 |
| 22 | 输入框 | 自实现;**拒绝** `ui_input::InputField` | `ui_input` 强制引入 zed 的 `editor` crate 整条栈 |
| 23 | Phase 1 范围 | 把 EPUB/TXT/FB2 **只读**元数据 + 封面生成提前到 Phase 1 | Phase 1 的验收要求"元数据正确 + 能编辑封面",离不开格式解析 |
| 24 | 书籍详情是否内联编辑 | 不做,只做模态对话框 | 与上游一致 |
| 25 | 默认排序 | `timestamp` **降序**(最新在前) | 上游 `DEFAULT_SORT = ('timestamp', False)`,第二项是 ascending 标志 |
| 26 | **许可证** | **整仓 GPL-3.0-or-later** | **用户已拍板**。calibre 是 GPLv3,zed 的 `ui`/`theme`/`icons` 同为 GPL-3.0-or-later,图标直接取自 calibre → 整体按 GPL-3.0-or-later 分发。字体 Noto Sans SC 为 OFL 1.1,单独声明 |
| 27 | **PDF** | **暂不做,留占位符** | **用户已拍板**("不着急,可以先放占位符")。格式注册表里保留 `pdf` 的占位插件(读元数据可返回 `Unsupported` 错误),接口与选项先留好,实现推迟到 Phase 3 之后按需补 |
| 28 | **图标** | **使用 calibre 原版**(`imgsrc/*.svg`,193 个) | **用户已拍板**。GPUI 原生支持 SVG,无需上游的 `rsvg-convert` 栅格化步骤;复制进本仓库 `assets/icons/` 以保持自包含,并在 `assets/NOTICE` 记录来源与许可 |
| 29 | **EPUB 优先级** | **EPUB2 与 EPUB3 为 P0**,排在所有格式之前 | **用户已拍板**。读取 Phase 1 就要支持两版;写入 Phase 2 支持两版,默认输出 EPUB2(与上游一致),EPUB3 需显式指定 |

---

## 9. 未决问题(需要人来拍板)

**原 3 项已全部拍板(见 §8 决策 26/27/28):许可证 = GPL-3.0-or-later;PDF = 暂不做、留占位符;图标 = calibre 原版。**

**仍然开放(不阻塞任何阶段,但越早定越省事):**

1. **中文字体的具体选择**(用户已同意"自带中文字体",但具体哪一款)
   候选:**Noto Sans SC**(SIL OFL 1.1,字重齐全,最稳)、**思源黑体 / Source Han Sans**(OFL,与 Adobe 联合开发,字形更精致但文件更大)、**HarmonyOS Sans**(免费商用但非开源,条款不明确)。
   **建议 Noto Sans SC**:许可最干净、字重齐全、subsets 工具成熟(可裁剪到 ~5 MB 的常用字集)。
2. **深色主题的图标适配**
   calibre 的图标有 `-for-dark-theme` / `-for-light-theme` 变体,也有自动反色的 `modified-for-*`。我们照搬 calibre 的变体规则即可,但**要不要额外提供"跟随 UI 主题自动切换"**(即把图标按当前主题重新着色)尚未定。
   **建议:先照搬 calibre 的变体选择规则**(见 D12),不做自动着色;若深色主题下观感不好再议。
3. **仓库名与二进制名**
   目前用 `calibre2Rust` 作为目录名。二进制名 `calibre2rust`?`calibre-rs`?还是与上游对齐叫 `calibre`?
   **建议 `calibre2rust`**(与目录名一致,且不与系统里的 `calibre` 冲突)。

---

## 10. 附:关键上游坐标索引

供实现者查阅 `../calibre`:

| 主题 | 位置 |
|---|---|
| 字段元数据(内置 + 自定义列) | `src/calibre/library/field_metadata.py` |
| 字段对象与分类 | `src/calibre/db/fields.py` |
| 搜索语言(语义) | `src/calibre/db/search.py` |
| 搜索语言(词法/语法) | `src/calibre/utils/search_query_parser.py` |
| 分类/标签浏览器数据 | `src/calibre/db/categories.py` |
| 写库路径 | `src/calibre/db/write.py`, `src/calibre/db/cache.py` |
| 库后端(SQLite) | `src/calibre/db/backend.py`, `src/calibre/db/schema_upgrades.py` |
| 主窗口布局 | `src/calibre/gui2/central.py`, `src/calibre/gui2/layout.py` |
| 标签浏览器 | `src/calibre/gui2/tag_browser/` |
| 书列表 | `src/calibre/gui2/library/views.py`, `models.py` |
| 封面网格 / 书架 | `src/calibre/gui2/library/alternate_views.py`, `bookshelf_view.py` |
| 书籍详情 | `src/calibre/gui2/book_details.py` |
| 搜索框与限制 | `src/calibre/gui2/search_box.py`, `search_restriction_mixin.py` |
| 工具栏 / 菜单 | `src/calibre/gui2/bars.py`, `src/calibre/gui2/__init__.py` |
| 偏好设置 | `src/calibre/gui2/preferences/` |
| 元数据模型 | `src/calibre/ebooks/metadata/book/base.py`, `metadata/__init__.py` |
| OPF2 | `src/calibre/ebooks/metadata/opf2.py` |
| 格式插件 trait | `src/calibre/customize/conversion.py`, `customize/__init__.py` |
| **格式插件实现(9.x 已集中)** | `src/calibre/ebooks/conversion/plugins/{format}_{input,output}.py` —— `ebooks/{epub,mobi,fb2,...}/` 下**已无插件代码** |
| 转换管线 | `src/calibre/ebooks/conversion/plumber.py`, `cli.py` |
| 格式底层实现 | `src/calibre/ebooks/{epub,mobi,fb2,html,rtf,odt,comic,pdf,...}/` |
| **EPUB2 → EPUB3 升级** | `src/calibre/ebooks/oeb/polish/upgrade.py`(`epub_2_to_3:152` / `collect_properties:67` / `create_nav:140`) |
| EPUB 容器读写 | `src/calibre/ebooks/epub/__init__.py`(`initialize_container`) |
| **图标源文件** | `calibre/imgsrc/*.svg`(193 个,含 `imgsrc/plugins/`) |
| 图标栅格化脚本(我们不需要) | `calibre/imgsrc/generate.py` |
| 封面生成 | `src/calibre/ebooks/covers.py` |
| 内容服务器 | `src/calibre/srv/` |
| CLI 命令 | `src/calibre/db/cli/`, `src/calibre/ebooks/conversion/cli.py` |
| 搜索语法手册 | `manual/gui.rst`(第 380–400 行有 `=.` / `=..` 的权威示例) |

GPUI 侧:

| 主题 | 位置(`../zed`) |
|---|---|
| 应用启动 | `crates/gpui/examples/hello_world.rs`, `crates/gpui_platform/src/gpui_platform.rs` |
| 输入框参考实现 | `crates/gpui/examples/input.rs` |
| 表格组件 | `crates/ui/src/components/data_table.rs` |
| 弹窗 / 菜单 / 提示 | `crates/ui/src/components/` |
| actions / keymap | `crates/gpui/src/action.rs`, `keymap/` |
| 拖拽 | `crates/gpui/examples/drag_drop.rs` |
| 自绘 canvas | `crates/gpui/examples/painting.rs` |
| 富文本 | `crates/gpui/src/elements/text.rs`(`StyledText` / `TextRun` / `TextLayout`) |
| 资源接口 | `crates/gpui/src/assets.rs`(`AssetSource`) |
| 主题初始化 | `crates/theme/src/theme.rs`(`theme::init`) |
| 主题 provider trait | `crates/theme/src/theme_settings_provider.rs` |
| 树形行组件 | `crates/ui/src/components/tree_view_item.rs` |
| Metal shader 编译 | `crates/gpui_apple/build.rs`(`runtime_shaders` 分支在 `:106-121`) |
