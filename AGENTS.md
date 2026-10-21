# calibre4rust — Agent 指南

**calibre4rust 是 [caliberate](https://github.com/sguzman/caliberate) 的一个 GPUI 桌面前端。**

用 [GPUI](https://github.com/zed-industries/zed)(Zed 的 UI 框架,git 依赖)实现 calibre 主界面的客户端;书库、搜索、分类、导入、转换、OPDS 等后端能力**全部复用 caliberate**,本仓库不重复实现。

本文件是编码 agent 的入口。**保持简短**;仓库文档才是准绳。

> 本文件是**仓库级**规则。若在多仓库工作区(`../calibre`、`../caliberate`、`../zed` 为只读参考)中工作,工作区根另有一份 `AGENTS.md` 覆盖跨仓库规则 —— 两份互补,不冲突。

---

## 三个上游仓库(全部是只读消费)

| 仓库 | 位置 | 角色 |
|---|---|---|
| **calibre** | `../calibre` | Python 版 calibre 源码。**GUI 行为基准** —— 交互模型、信息架构逐项对照 `src/calibre/gui2/` |
| **caliberate** | `../caliberate` | Rust 版 calibre 平台,本项目的后端。**只读**,不改;发现缺口记入 `docs/caliberate-gaps.md` |
| **zed** | `../zed` | GPUI 来源。**只读**,不改 |

三者均为 git 依赖 + 固定 rev(见根 `Cargo.toml`),因此**不依赖**兄弟目录的当前状态。

---

## 动手前必读

1. `docs/superpowers/specs/2026-10-21-calibre4rust-design.md` —— 设计文档
2. `docs/superpowers/plans/2026-10-21-phase-abc-gpui-frontend.md` —— 当前实施计划
3. caliberate 的架构边界:`../caliberate/ARCHITECTURE.md`、`../caliberate/docs/project/priorities.md`
4. 上游 calibre 的对应模块(见设计文档 §10 坐标索引)

**不要**从旧 tranche / parity 文档推断权威性。`docs/project/` 与 `docs/work/` 是活的协调层。

---

## 当前计划

工作按 `docs/superpowers/plans/` 里的计划推进,一次一个 task,每个 task 自带测试循环与提交。

执行方式二选一:
- **子代理驱动**(`superpowers:subagent-driven-development`)—— 每个 task 派新代理实现,再派新代理审查
- **自执行**(`superpowers:executing-plans`)—— 本会话内逐 task 实现,最后整体审查

**不要**把多个 Phase 塞进一份计划。Phase A/B/C 在 `2026-10-21-phase-abc-gpui-frontend.md`;后续阶段(分类浏览器+搜索、书籍详情+封面网格、元数据编辑、工具栏动作+首选项、阅读器)各自另出计划。

---

## 构建与验证

```bash
# 工具链(直连 rust-lang.org 会超时,必须走镜像)
RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1

# 依赖与构建 —— crates.io 直连极慢,必须走镜像
cargo fetch --target aarch64-apple-darwin \
  --config 'source.crates-io.replace-with="rsproxy"' \
  --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'

cargo build --target aarch64-apple-darwin
cargo test  --target aarch64-apple-darwin
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

建议把 crates.io 镜像写进 `~/.cargo/config.toml`(全局)。**不要**写进仓库的 `.cargo/config.toml`(保持仓库可移植)。

**禁止 `cargo metadata`** —— 它会解析所有 target 的依赖(本机实测 >15 分钟未完成)。

---

## 硬性约束(踩过的坑,别再踩)

| 约束 | 原因 |
|---|---|
| 根 `Cargo.toml` 必须重复 `[patch.crates-io] async-task` | cargo **只读消费方 workspace 根**的 patch,依赖内的 patch 被忽略。zed 用了 `async-task` 的 fork(API 泛型化为 `Task<T, M>`),缺了它 `gpui` 编译失败 |
| `gpui_platform` 必须开 `runtime_shaders` | 本机无 Metal Toolchain,`xcrun metal` 不可用;`gpui_apple/build.rs:161-181` 会 `process::exit(1)` 直接构建失败。`runtime_shaders` 让 shader 运行时编译,绕开它 |
| `gpui_platform` 必须开 `font-kit` | macOS 上它 gate 整个 Apple 文本系统,不开则一个字都渲染不出来 |
| `gpui` / `gpui_platform` 必须 `default-features = false` | `gpui_platform` 的 `default = ["multithreaded"]` 指向 wasm |
| 工具链固定 `1.98.1` | zed 的要求 |
| `.cargo/config.toml` 必须设 `MACOSX_DEPLOYMENT_TARGET = "10.15.7"` | 与 zed 对齐 |

---

## 架构红线

1. **只有 `service.rs` 可以 import `caliberate_*` crate。** 其它模块一律不碰 `caliberate-db` / `caliberate-library` / `rusqlite`。**有架构测试强制**(`crates/calibre4rust-gui/src/lib.rs` 的 `architecture_tests`)。
   理由:caliberate 的 `ARCHITECTURE.md` 要求 *"Protocol handlers must not independently reimplement database queries, search rules, or path-selection logic."*
2. **发现 caliberate 的 service 缺口,登记到 `docs/caliberate-gaps.md`,不在前端绕过。** 不改 `../caliberate` 仓库 —— 那是它的架构师的事。
3. **`LibraryCatalog<'a>` 借用 `&'a dyn LibraryBackend`,不能自引用。** 照搬 caliberate egui 侧的做法(`crates/gui/src/views.rs:90`):每次调用现建 `LibraryCatalog::new(&self.database)`。**不要**用 `Box::leak` 或自引用结构绕过。
4. **GPUI 只是呈现层。** 任何"图省事在 UI 里直接查 SQLite"的做法都违反上一条。

---

## Rust 规范

沿用 zed 的 `AGENTS.md` 与 caliberate 的 `AGENTS.md`:

- 禁止 `unwrap()`(用 `?` 传播);禁止 `let _ =` 丢弃可失败操作;禁止"总结代码"的注释
- 变量用完整词,不用缩写
- **禁止 `mod.rs`**;crate 用 `[lib] path = "src/lib.rs"`
- 用 `tracing` 做运行时诊断,不要 `println!`
- `cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check` 必须通过
- 改不动的既有 warning 存量不转为本项目范围,但**本 crate 自己的 warning 必须清零**

---

## 上游事实速查(调研结论,别重新踩)

### calibre(Python,行为基准)

- **搜索运算符顺序**:`=.value` / `=..value`,**等号在前**。写成 `.=value` 会退化成字面包含,静默匹配不到任何东西。依据 `db/search.py:36-55` 与 `manual/gui.rst:390-400`
- **默认排序**:`timestamp` **降序**(最新在前)。`DEFAULT_SORT = ('timestamp', False)` 的第二项是 **ascending 标志**。依据 `db/cache.py:1768-1776` 与 `gui2/library/models.py:1906`
- **macOS 工具栏默认项**:**没有** Help / Preferences(它们在原生菜单栏)。依据 `gui2/__init__.py:359-376`;非 macOS 的分支在 `:399-417`
- **主窗口**:5 面板 + 4 分隔条;拖过 10px 隐藏面板、超过 50px 恢复。依据 `gui2/central.py:39-40`, `:394-397`
- **格式插件已集中**到 `ebooks/conversion/plugins/{format}_{input,output}.py`;`ebooks/{epub,mobi,fb2,...}/` 下**已无插件代码**。AZW3 由 MOBI 插件处理,AZW 上游**无插件**
- **图标源**:`imgsrc/*.svg`(193 个,含 `imgsrc/plugins/`);GPUI 原生支持 SVG,**不需要**上游的 `rsvg-convert` 栅格化
- **深色主题图标变体**:图标名 N 先找 `N-for-{主题}-theme.svg`,找不到则用 `N.svg`;另有自动反色的 `modified-for-{主题}-theme.svg`
- 分类浏览器有 9 类;caliberate 的 `LibraryFacetKind` 只提供 6 类(缺 Formats / Identifiers)

### caliberate(Rust,后端)

- service 入口:`crates/library/src/catalog.rs` 的 `LibraryCatalog` facade over `LibraryBackend` trait
- 查询 / facet / 排序:`crates/library/src/query.rs`
- 列表摘要:`crates/library/src/summary.rs` 的 `LibraryBookSummary`
- calibre 库只读挂载:`crates/library/src/calibre/` 的 `CalibreLibraryBackend`
- **macOS 编译已验证通过**(2026-10-21,全 14 crate `cargo check --workspace --all-targets` 零错误)
- egui 前端(`crates/gui/src/views.rs`,13,628 行)是**参考实现**,不是依赖

### GPUI(zed)

- 启动入口:`gpui_platform::application()`(不是 `Application::new()`)
- 表格:`ui::Table`(`crates/ui/src/components/data_table.rs:356`)
- **没有** tree / input / table 原语 —— 树与输入框自实现;输入框参考 `crates/gpui/examples/input.rs`
- **没有** `ThemeSettingsProvider` 就渲染 `ui` 组件会 **panic**;必须自实现该 trait(5 个方法)并 `theme::init(LoadThemes::JustBase, cx)`
- `assets` crate(内嵌 Zed 字体图标)**不引入** —— 无中文字形;自建 `AssetSource`,字体用 macOS 系统字体 + `FontFallbacks` fallback
- `ui_input::InputField` **不用** —— 它要求 `ERASED_EDITOR_FACTORY`,会拖入 zed 的 `editor` 整条栈

---

## 提交

每个 task 结束提交一次,commit message 用中文描述做了什么。

不要替用户 merge / push 到受保护分支;不要改 `../caliberate` 或 `../zed`。
