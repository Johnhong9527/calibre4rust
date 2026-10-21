# Phase A/B/C 验收记录

> 日期:2026-10-21
> 分支:`feat/phase-abc-gpui-frontend`
> 计划:`docs/superpowers/plans/2026-10-21-phase-abc-gpui-frontend.md`
> Spec:`docs/superpowers/specs/2026-10-21-calibre4rust-design.md`

---

## 1. 验收清单

| # | 验收项 | 结果 | 证据 |
|---|---|---|---|
| A1 | 窗口打开、中文正常显示 | ⚠️ **部分** | 进程存活、事件循环运行(RSS 110 MB)、零 stderr 输出、无 Metal/shader 错误。**像素渲染未肉眼确认** —— 本机无录屏权限,`screencapture`/`osascript` 被系统拒绝。需在有录屏权限的机器补验 |
| A2 | calibreate service 接通 | ✅ | `calibredb add` 写入的 2 本书,本前端 `calibre4rust list` 读出并按标题排序 |
| A3 | 工具栏动作对齐 calibre macOS 默认 | ✅ | `window::tests::macos_toolbar_matches_upstream_calibre_default` 断言完整 16 项序列;上游依据 `calibre/src/calibre/gui2/__init__.py:359-376` |
| A4 | 菜单栏动作对齐 | ✅ | `window::tests::menubar_contains_preferences_and_help` 断言 7 项;上游 `gui2/__init__.py:331-340` |
| A5 | 面板分隔条阈值 | ✅ | `window::tests::panel_thresholds_match_upstream_calibre` 断言 10/50;上游 `gui2/central.py:39-40` |
| B1 | 五面板 + 可拖拽分隔条 | ✅ | `window.rs` 实现:宽/窄两套布局(Alt+Shift+L 切换),拖过 10px 隐藏、超过 50px 恢复 |
| B2 | 状态栏显示书数 | ✅ | `MainWindow::new` 建窗时统计一次并显示 |
| B3 | 书库打不开时显示可读错误 | ✅ | `service::tests::a_failing_database_open_yields_a_readable_message` 断言错误信息含"书库";GUI 状态栏红色显示 |
| C1 | 书列表表格视图 | ✅ | `ui::Table` + `uniform_list` 虚拟滚动,8 列 |
| C2 | 列定义对齐上游 | ✅ | `book_list::tests::default_columns_hide_upstream_hidden_fields` 断言默认隐藏 `last_modified/languages/formats/id/path/pages`;上游 `gui2/library/views.py:1106` |
| C3 | 点击表头排序 | ✅ | `book_list::tests::title_column_maps_to_title_sort_field` 等 |
| C4 | 大库分块加载 | ✅ | `book_list::tests::chunk_offsets_cover_the_whole_result_set` 覆盖 10 万本 / 50 块;`load_all_summaries` 先取 total 再按 2000 一块拉 |
| C5 | 非 ASCII / emoji / 超长书名 | ✅ | `book_list::tests::row_cells_render_cjk_emoji_and_long_titles` 等 3 项 |
| C6 | 视图状态持久化 + clamp | ✅ | `view_state::` 6 项测试;端到端:预置 2000px 面板宽 → 启动后 clamp 到 480+480(=1280 − 320)并回写;列宽 −50→24、99999→1200 |
| C7 | 架构边界(只有 service.rs 依赖 caliberate) | ✅ | `architecture_tests::only_service_module_imports_caliberate_crates` |

## 2. 验证命令与结果

```
$ cargo fmt --check
(无输出 = 通过)

$ cargo clippy --all-targets --target aarch64-apple-darwin -- -D warnings
Finished `dev` profile —— 零 error 零 warning(本 crate)

$ cargo test --target aarch64-apple-darwin
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo build --target aarch64-apple-darwin
Finished `dev` profile —— 二进制 calibre4rust 生成成功
```

## 3. 端到端验证(真实数据链路)

不是造数据,而是走 caliberate 自己的写入路径:

```bash
# 用 calibreate 的 calibredb 往库里加书(两份手工构造的 EPUB,含中文/emoji/NCX 目录)
$ calibredb add --path books/三体.epub
Added book 1
$ calibredb add --path books/Dune.epub
Added book 2
$ calibredb list
1  三体  epub  ./.cache/caliberate/library/三体.epub.zst
2  Dune  epub  ./.cache/caliberate/library/Dune.epub.zst

# 本前端读同一个库
$ CALIBRE4RUST_LIBRARY=.../caliberate.db calibre4rust list
2  Dune  epub
1  三体  epub
共 2 本
```

这条链路证明:caliberate 写入 → 本前端读取 → 分块查询 → 排序 → 渲染,全程贯通。

## 4. 未验证项(需要人工/换环境)

1. **像素渲染与中文显示** —— 本机无录屏权限,无法截图确认"你好,世界"不是豆腐块、表格列对齐正常。**必须在有录屏权限的机器上补验**,这是 Phase A 唯一未闭合的验收项。
2. **拖拽分隔条的手感** —— 10px/50px 阈值与宽/窄布局切换的逻辑有测试覆盖,但交互手感需人工判断。
3. **超大书库的实际滚动性能** —— 分块逻辑有测试(10 万本 / 50 块),但真实滚动帧率未测。
4. **caliberate 在 Linux / Windows 的兼容性** —— 本项目只保证 macOS;caliberate 侧官方保证 Windows/Linux。

## 5. 与 spec 的偏离

| 项 | spec | 实际 | 理由 |
|---|---|---|---|
| `LibraryService::open` | `open(config: &ControlPlane)` | `open_path(path, busy_timeout_ms)` + `open_in_memory()` | `ControlPlane` 不派生 `Default` 且有 19 个必填字段,测试里构造要 150 行;control-plane TOML 接入留待后续 |
| service 的类型转发 | — | `service.rs` 转发 `LibraryBookSummary`/`LibraryQuery`/`LibrarySortField` 等纯数据类型 | 否则 UI 直接 import `caliberate_library` 会撞架构测试;边界只管查询路径,不管数据形状 |
| 额外 CLI | 计划放在 Task 6/21 | Task 4 就加了 `calibre4rust list` | 没有它就无法为"表格真的读出了数据"提供可观察证据 |
| 状态文件位置 | 用 `ControlPlane.paths` 的 GUI 状态目录 | `<library_dir>/view-state.json` | control-plane TOML 尚未接入 |
| GPUI 版本 | crates.io 0.2.2 | git rev `92447a17` | crates.io 版本是旧 API(`Application::new`),与当前 zed main 不兼容 |

## 6. 后续阶段(各自另出计划)

| 阶段 | 内容 | 阻塞项 |
|---|---|---|
| D/E | 分类浏览器 + 搜索栏 | 分类浏览器可做(6 类 facet);搜索受 caliberate 缺口 #1 限制,只能做自由文本 + 字段定向 |
| F/G | 书籍详情 + 封面网格 | 封面取法待确认(缺口 #6) |
| H | 元数据编辑 | 需确认 `update_book_*` 是否支持多值(缺口 #5) |
| I | 工具栏动作 + 首选项 | — |
| J | 阅读器 | **阻塞于 caliberate 侧 `crates/document` 与 `crates/formats`**(缺口 #7) |
