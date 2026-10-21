# calibre4rust — Agent 指南

**calibre4rust 是 [caliberate](https://github.com/sguzman/caliberate) 的一个 GPUI 桌面前端。**

用 [GPUI](https://github.com/zed-industries/zed)(Zed 的 UI 框架)实现 calibre 主界面的客户端;书库、搜索、分类、导入、转换、OPDS 等后端能力**全部复用 caliberate**,本项目不重复实现。

本文件是编码 agent 的入口。**保持简短**;仓库文档才是准绳。

> 本文件是**仓库级**规则。若在多仓库工作区(`../calibre`、`../zed` 为只读参考)中工作,工作区根另有一份 `AGENTS.md` 覆盖跨仓库规则 —— 两份互补,不冲突。

---

## 仓库结构

```
calibre4rust/
├── caliberate/          ← git 子模块,可写(见下)
├── crates/calibre4rust-gui/   ← 本项目唯一的 crate
├── assets/              ← 图标(取自 calibre)与字体
├── docs/superpowers/    ← 设计文档、实施计划、验收记录、ledger
└── tools/
```

| 依赖 | 形式 | 说明 |
|---|---|---|
| `gpui` 等 10 个 zed crate | **git 依赖**,固定 rev `92447a17` | 跟随 zed main |
| `caliberate-core` / `-db` / `-library` | **path 依赖** → `caliberate/crates/*` | 子模块指针锁定 commit,构建可复现 |

**首次 clone 必须**:
```bash
git submodule update --init --recursive
```

---

## caliberate 子模块:可以改,但要守它的规矩

`caliberate/` 是 git 子模块,指向 **`github.com/Johnhong9527/caliberate`**(我们 `sguzman/caliberate` 上游的 fork —— 我们对上游**只有读权限**,推不上去)。

- **可以**在子模块里改代码 —— 补 service 能力缺口(搜索语法、多级排序、更多 facet、写入操作暴露、封面 API)就是干这个的。
- 改动要推到 **fork**(`Johnhong9527/caliberate`),然后更新本仓库的子模块指针。
- caliberate **保留它自己的治理**,不要绕过:
  - 工作项协议:`docs/work/ready/` → `active/` → `done/`,**同时只能有一个在 ready/**。
  - 分支 `codex/<task-id>-<slug>`,task ID 形如 `0027-<slug>`(补丁号 `0027.1`)。
  - 报告写 `docs/work/reports/<task-id>.md`,**明确写出没验证什么**。
  - 工作项文件必含:`## Goal`、`## N.` 编号小节、`## Architecture constraints`、`## Explicit non-goals`、`## Expected files`、`## Validation`、`## Handoff`。
  - `## Validation` 写平台限定的命令块,结尾写 "All must pass."
  - 错误用 `CoreError`/`CoreResult`(**不用 anyhow**),消息小写无句号;rusqlite 错误必须包一层可诊断 label(`"prepare X"` / `"query X"` / `"read X"`)。
  - 库 crate 里**禁止 `println!`**,用 `tracing` 或暴露 progress 回调。
  - 新模块控制在 **200–550 行**;**不要**往 `crates/db/src/database.rs`(5.3k 行)、`crates/gui/src/views.rs`(13.6k 行)里塞新逻辑。
  - 代码注释与文档**全英文**;每个新文件第一行一句 `//!` 模块说明。

---

## 另外两个上游:只读

| 仓库 | 位置 | 角色 |
|---|---|---|
| **calibre** | `../calibre` | Python 版 calibre 源码。**GUI 行为基准** —— 交互模型、信息架构逐项对照 `src/calibre/gui2/`。**不要改** |
| **zed** | `../zed` | GPUI 来源。**不要改** |

---

## 动手前必读

1. `docs/superpowers/specs/2026-10-21-calibre4rust-design.md` —— 设计文档
2. `docs/superpowers/plans/` —— 当前实施计划与 ledger
3. caliberate 的架构边界:`caliberate/ARCHITECTURE.md`、`caliberate/docs/project/priorities.md`
4. 上游 calibre 的对应模块(见设计文档 §10 坐标索引)

**不要**从旧 tranche 或 parity 文档推断权威性。`docs/project/` 与 `docs/work/` 是活的协调层。

---

## 当前计划

工作按 `docs/superpowers/plans/` 里的计划推进,一次一个 task,每个 task 自带测试循环与提交。

执行方式二选一:
- **子代理驱动**(`superpowers:subagent-driven-development`)—— 每个 task 派新代理实现,再派新代理审查
- **自执行**(`superpowers:executing-plans`)—— 本会话内逐 task 实现,最后整体审查

**不要**把多个 Phase 塞进一份计划。Phase A/B/C 已完成(见 `docs/superpowers/specs/2026-10-21-phase-abc-acceptance.md`);后续阶段各自另出计划。

---

## 构建与验证

```bash
# 工具链(直连 rust-lang.org 会超时,须走镜像)
RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1

# 依赖与构建(crates.io 直连极慢,须走镜像)
cargo fetch --target aarch64-apple-darwin \
  --config 'source.crates-io.replace-with="rsproxy"' \
  --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'

cargo build --target aarch64-apple-darwin
cargo test  --target aarch64-apple-darwin
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

建议把 crates.io 镜像写进 `~/.cargo/config.toml`(全局)。**不要**写进仓库的 `.cargo/config.toml`(保持仓库可移植)。

**caliberate 子模块单独验证**(它有自己的 workspace 与约定,本仓库的 `exclude = ["caliberate"]` 把它挡在外面):
```bash
cargo test --manifest-path caliberate/Cargo.toml --workspace
```

**禁止 `cargo metadata`** —— 它会解析所有 target 的依赖(本机实测 >15 分钟未完成)。

---

## 硬性约束(踩过的坑,别再踩)

| 约束 | 原因 |
|---|---|
| 根 `Cargo.toml` 必须重复 `[patch.crates-io] async-task` | cargo **只读消费方 workspace 根**的 patch,依赖内的 patch 被忽略。zed 用了 `async-task` 的 fork(API 泛型化为 `Task<T, M>` / `Runnable<M>`),缺了它 `gpui` 编译失败 |
| `gpui_platform` 必须开 `runtime_shaders` | 本机无 Metal Toolchain,`xcrun metal` 不可用;`gpui_apple/build.rs:161-181` 会 `process::exit(1)` 构建失败 |
| `gpui_platform` 必须开 `font-kit` | macOS 上它 gate 整个 Apple 文本系统,不开则一个字都渲染不出来 |
| `gpui` / `gpui_platform` 必须 `default-features = false` | `gpui_platform` 的 `default = ["multithreaded"]` 指向 wasm |
| 工具链固定 `1.98.1` | zed 的要求 |
| `.cargo/config.toml` 必须设 `MACOSX_DEPLOYMENT_TARGET = "10.15.7"` | 与 zed 对齐 |
| **`exclude = ["caliberate"]`** | 否则 cargo 把工作目录内的路径依赖自动并入成员,本仓库的 rustfmt.toml 会去改写 caliberate 的代码 |

---

## 架构红线

1. **只有 `service.rs` 可以依赖 caliberate 的 crate。** 其它模块一律不碰 `caliberate-db` / `caliberate-library` / `rusqlite`。**有架构测试强制**(`crates/calibre4rust-gui/src/lib.rs` 的 `architecture_tests`):全文件扫描 + 运行时拼接目标串,`pub use` 与内联全路径都逃不掉。
2. **发现 caliberate 的 service 缺口,登记到 `docs/caliberate-gaps.md`,不在前端绕过。** 现在可以在子模块里补 —— 但**按 caliberate 的工作项协议走**,不要在子模块里做架构变更。
3. **`LibraryCatalog<'a>` 借用 `&'a dyn LibraryBackend`,不能自引用。** 照搬 caliberate egui 侧的做法(`crates/gui/src/views.rs:90`):每次调用现建 `LibraryCatalog::new(&self.database)`。**不要**用 `Box::leak` 或自引用结构绕过。
4. **GPUI 只是呈现层。** 任何"图省事在 UI 里直接查 SQLite"的做法都违反上一条。

---

## Rust 规范

沿用 zed 的 `AGENTS.md`(前端)与 caliberate 的 `AGENTS.md`(子模块内代码):

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
- **主窗口**:5 面板 + 4 分隔条;拖分隔条低于 10px 隐藏面板、超过 50px 恢复。依据 `gui2/central.py:39-40`, `:394-397`
- **格式插件已集中**到 `ebooks/conversion/plugins/{format}_{input,output}.py`;`ebooks/{epub,mobi,fb2,...}/` 下**已无插件代码**。AZW3 由 MOBI 插件处理,AZW 上游**无插件**
- **图标源**:`imgsrc/*.svg`(193 个,含 `imgsrc/plugins/`);GPUI 原生支持 SVG,**不需要**上游的 `rsvg-convert` 栅格化
- **深色主题图标变体**:图标名 N 先找 `N-for-{主题}-theme.svg`,找不到则用 `N.svg`;另有自动反色的 `modified-for-{主题}-theme.svg`
- 分类浏览器有 9 类;caliberate 的 `LibraryFacetKind` 只提供 6 类(缺 Formats / Identifiers)

### caliberate(Rust,后端 —— 现在可改)

- service 入口:`crates/library/src/catalog.rs` 的 `LibraryCatalog` facade over `LibraryBackend` trait
- 查询 / facet / 排序:`crates/library/src/query.rs`
- 列表摘要:`crates/library/src/summary.rs` 的 `LibraryBookSummary`
- calibre 库只读挂载:`crates/library/src/calibre/` 的 `CalibreLibraryBackend`
- **已知缺口**(详见 `docs/caliberate-gaps.md`):完整搜索语法缺失、facet 少 2 类、单级排序、`LibraryBackend` 只读、单项 tag/语言增删缺失、封面无存储 API、`crates/formats` 全是 1 行占位、无 `crates/document`
- egui 前端(`crates/gui/src/views.rs`,13,628 行)是**参考实现**,不是依赖

### GPUI(zed)

- 启动入口:`gpui_platform::application()`(不是 `Application::new()`)
- 表格组件:`ui::Table`(`crates/ui/src/components/data_table.rs`)
- 弹窗 / 菜单 / 提示:`crates/ui/src/components/`
- actions / keymap:`crates/gpui/src/action.rs`, `keymap/`
- 拖拽:`crates/gpui/examples/drag_drop.rs`
- 自绘 canvas:`crates/gpui/examples/painting.rs`
- 富文本:`crates/gpui/src/elements/text.rs`(`StyledText` / `TextRun` / `TextLayout`)
- 资源接口:`crates/gpui/src/assets.rs`(`AssetSource`)
- 主题初始化 / provider:`crates/theme/src/theme.rs`(`theme::init`), `theme_settings_provider.rs`
- 树形行组件:`crates/ui/src/components/tree_view_item.rs`
- Metal shader 编译:`crates/gpui_apple/build.rs`(`runtime_shaders` 分支)

---

## 提交

每个 task 结束提交一次,commit message 用中文描述做了什么。

不要替用户 merge / push 到受保护分支;`calibre/` 与 `../zed` 不要改。
