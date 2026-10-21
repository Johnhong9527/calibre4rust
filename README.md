# calibre4rust

calibre 的 **GPUI 桌面前端**。

用 [GPUI](https://github.com/zed-industries/zed)(Zed 的 UI 框架)实现 calibre 主界面的客户端;书库、搜索、分类、导入、转换、OPDS 等后端能力**全部复用 [caliberate](https://github.com/sguzman/caliberate)**,本项目不重复实现。

> **为什么不是从零写?**
> caliberate 已有 47,687 行、14 个 crate 的成熟实现(SQLite 持久化、`LibraryCatalog` service、calibre 库只读挂载、OPDS 服务)。重复造轮子还会违反 caliberate 自己的架构边界 —— *"Protocol handlers must not independently reimplement database queries, search rules, or path-selection logic."*
>
> **为什么用 GPUI?**
> caliberate 现有 GUI 是 eframe/egui,`views.rs` 有 13,628 行,是它自己承认的 *"legacy concentration"*。用 GPUI 在独立工程里重做,不必推倒既有实现。
>
> **calibre(Python)在其中的角色**是**行为基准** —— 交互模型与信息架构的参照,不是运行时依赖。

---

## 当前进度

Phase A/B/C **已完成**(验收记录见 `docs/superpowers/specs/2026-10-21-phase-abc-acceptance.md`):

| 阶段 | 内容 | 状态 |
|---|---|---|
| A | 依赖打通(GPUI + caliberate git 依赖)、GPUI 启动、service 接入 | ✅ |
| B | 主窗口骨架(工具栏 / 菜单栏 / 搜索栏 / 五面板 / 分隔条) | ✅ |
| C | 书列表表格视图(分块加载 + 排序 + 多选) | ✅ |

已验证:caliberate 的 `calibredb` 写入的书,本前端能读出并渲染(`calibre4rust list`)。
未验证:像素渲染与中文显示(本机无录屏权限,需人工补验)。

后续:分类浏览器 + 搜索栏、书籍详情 + 封面网格、元数据编辑、工具栏动作 + 首选项、阅读器。
阅读器阻塞于 caliberate 侧的 `crates/document` 与 `crates/formats`(见 `docs/caliberate-gaps.md`)。

---

## 构建

```bash
# 工具链(1.98.1,直连 rust-lang.org 会超时,须走镜像)
RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static rustup toolchain install 1.98.1

# 依赖(crates.io 直连极慢,须走镜像;建议把镜像写进 ~/.cargo/config.toml)
cargo fetch --target aarch64-apple-darwin \
  --config 'source.crates-io.replace-with="rsproxy"' \
  --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'

cargo build --target aarch64-apple-darwin
cargo test  --target aarch64-apple-darwin
```

运行:`./target/aarch64-apple-darwin/debug/calibre4rust`

**不要用 `cargo metadata`** —— 它会解析所有 target 的依赖。

---

## 依赖的上游

| 仓库 | 用途 |
|---|---|
| [zed-industries/zed](https://github.com/zed-industries/zed) @ `92447a17` | GPUI(`gpui` / `gpui_platform` / `ui` / `theme` / `icons` / `menu` / `fuzzy` / `component`) |
| [sguzman/caliberate](https://github.com/sguzman/caliberate) @ `f9c41d7` | 后端(`caliberate-core` / `caliberate-db` / `caliberate-library`) |

两者均为 git 依赖 + 固定 rev,不依赖本地检出状态。

---

## 许可

**GPL-3.0-or-later**。

- `assets/icons/` 下的 SVG 取自 calibre 的 `imgsrc/`,版权归 Kovid Goyal 及 calibre 贡献者所有,同样以 GPL-3.0-or-later 分发(见 `assets/NOTICE`)。
- 本项目是 calibre 的衍生实现,按上游 calibre 的许可分发。

---

## 给 agent 的入口

见 `AGENTS.md`(仓库级)与 `../AGENTS.md`(工作区级)。设计文档与实施计划在 `docs/superpowers/`。
