# caliberate service 缺口

本文件记录 calibre4rust(GPUI 前端)在对接 caliberate 时发现的 service 能力缺口。

**规则**:前端**不得**绕过 service 直接查 SQLite 来填补这些缺口。应在本文登记,由 caliberate 的架构师决定是否补齐。

---

## 已知缺口

| # | 缺口 | 影响 | 前端临时做法 | 状态 |
|---|---|---|---|---|
| 1 | 无 calibre 完整搜索语法(`=` `~` `^` `\` `=.` `=..`、三引号、日期比较、评分比较、三态布尔、`search:` / `vl:` / `@UserCategory:` 位置) | 无法提供与 calibre 等价的搜索 | 只做自由文本 + 字段定向(`title:` / `author:` / `tag:` / `series:` / `publisher:` / `format:`) | 待登记到 caliberate |
| 2 | `LibraryFacetKind` 缺 `Formats` / `Identifiers` | 分类浏览器比 calibre 少 2 类(calibre 有 9 类,caliberate 提供 6 类) | 分类浏览器只显示 6 类 | 待登记 |
| 3 | `LibraryQuery` 只支持**单级**排序 | 无法做 calibre 的多级排序(最多 3 级) | 主排序字段排序;次级 tie-breaker 缺失 | 待登记 |
| 4 | `LibraryBackend` trait 只有读方法 | 元数据编辑无法走 service,必须绕到 `Database::update_book_*` | `service.rs` 内部直接调 `Database`;仍不碰 SQL | 待登记 |
| 5 | `update_book_*` 是否支持**多值**字段(标签 / 系列 / 语言)未确认 | 批量编辑可能受阻 | 实现时确认;若只支持单值则登记为阻塞项 | 待确认 |
| 6 | 封面在 service API 中的暴露方式未确认 | 书籍详情面板取不到封面 | 实现时确认 `resolve_content` / assets 层;必要时登记 | 待确认 |
| 7 | 无 `crates/document`(normalized document model);`crates/formats` 仅占位 | 阅读器无输入 | Phase J 顺延,等 caliberate 侧就绪 | 阻塞中 |
| 8 | 自动封面生成是占位渲染器 | 封面质量差 | 前端可用 GPUI 画布增强(可选,非"复刻 calibre"范围) | 已知 |

## 平台缺口

| # | 缺口 | 结论 |
|---|---|---|
| P1 | caliberate 官方保证 Windows/Linux,macOS 未验证 | **已验证**(2026-10-21):`cargo check --workspace --all-targets` 在 macOS aarch64 上零错误通过,全部 14 个 crate 均可编译。警告仅来自既有 `caliberate-gui`(57 条 dead_code)与 `caliberate-library`(1 条),不属本项目范围 |

## 本前端自身的待办(不是 caliberate 的缺口)

- 搜索栏的字段定向解析需要把 `title:` 之类的语法映射到 `LibraryQuery` 的对应字段 —— 依赖缺口 #1 的部分能力。
- 多级排序的 tie-breaker 需要 caliberate 支持 —— 依赖缺口 #3。
- 封面取法待确认 —— 依赖缺口 #6。

## 本前端侧的自建缺口(Phase A/B/C 遗留)

| # | 缺口 | 影响 | 计划 |
|---|---|---|---|
| F1 | 未接 control-plane TOML(`ControlPlane`) | 配置只能靠 `CALIBRE4RUST_LIBRARY` 环境变量与默认值 | Phase I |
| F2 | 未做窗口关闭时的状态落盘(gpui 92447a1 无 `WindowEvent`) | 崩溃时最后一次状态可能丢失 | 状态每次渲染同步,实际风险低 |
| F3 | 像素渲染与**交互行为**未肉眼确认 | 整分支审查发现首版因缺 `cx.notify()` 导致拖拽/布局切换/排序全部失效(已修),但修复后仍未观察 | 见验收记录 §4,需有录屏权限的环境补验 |
| F4 | 列宽的持久化字段已就位但未接到 `ui::Table` 的列宽配置 | 恢复的列宽暂不生效 | Phase D/E |
| F5 | 隐藏列已接到 `BookList::apply_hidden_columns`,但 UI 没有开关 | 只能改配置文件 | Phase D/E |
| F6 | 工具栏按钮不触发 action | 工具栏目前是纯展示 | Phase I |
| F7 | calibre 书库只能被拒绝打开,还不能只读挂载 | 用户无法在本前端里浏览自己的 calibre 书库 | 依赖缺口 #6 / caliberate 的 `CalibreLibraryBackend` 接入 |

## 整分支审查(2026-10-21)已修复项

| # | 问题 | 修复 |
|---|---|---|
| C1 | 交互式状态变更从不调 `cx.notify()` → 拖拽/布局切换/排序在视觉上完全无效 | 所有变更处理器补 `cx.notify()` |
| C2 | 分隔条拖动无指针捕获 → 移出把手即中断,在别处松手不结束,留下过期拖动状态 | 在 `render` 里每帧注册窗口级 `MouseMoveEvent`/`MouseUpEvent`(gpui 的 `Window::on_mouse_event` 只活一帧) |
| I1 | 全量数据加载在 UI 线程;每次渲染克隆整份数据集(悬停即触发) | 计数改用 `page.total`;加载移到后台线程(另开只读连接);渲染按范围读取,不再全量克隆 |
| I2 | 多选完全缺失 | 实现 `selection_after_click`(普通 / Cmd 反选 / Shift 连选),行点击已接上 |
| I3 | 4 条分隔条里 2 条(横向)是装饰 | 封面浏览器 / 快速预览的高度也支持拖拽 |
| I4 | 几何收敛只缩小且被持久化 → 窗口缩小再放大永久丢失面板宽度 | 收敛改为只作用于渲染副本,不写回 |
| I5 | 架构测试可被 `pub use` 或内联全路径绕过 | 改为全文件扫描 + 运行时拼接目标串 |
| I6 | 打开任何 SQLite 文件都会跑 schema 迁移 → 指向 calibre 的 `metadata.db` 会改写用户数据 | 打开前检测 calibre 特征(有 `books` 表、无 `schema_migrations`)并拒绝;已验证文件字节前后一致 |
| I7 | 查询期错误(如库被占用)对用户不可见 | 错误存到 `BookList::last_error`,在状态栏与书列表页脚显示 |

