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
| F2 | 未做窗口关闭时的状态落盘(gpui 92447a1 无 `WindowEvent`) | 崩溃时最后一次状态可能丢失 | 状态每帧同步,实际风险低 |
| F3 | 像素渲染未肉眼确认 | 需有录屏权限的环境补验 | 见验收记录 §4 |
| F4 | 列宽的持久化字段已就位但未接到 `ui::Table` 的列宽配置 | 恢复的列宽暂不生效 | Phase D/E |

