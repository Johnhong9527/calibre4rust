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
| B1 | 五面板 + 可拖拽分隔条 | ⚠️ **代码就绪,未观察** | 4 条分隔条都有拖拽逻辑,拖动阈值 10px/50px 有测试覆盖。**但交互本身仍未被肉眼验证** —— 整分支审查发现首版实现从不调 `cx.notify()`,导致拖拽与布局切换在视觉上完全无效;该缺陷已修(见 §7),但**修复后仍需人工确认** |
| B2 | 状态栏显示书数 | ⚠️ **代码就绪,未观察** | 同上 —— 首版因缺 `cx.notify()` 而不刷新;已修,待人工确认 |
| B3 | 书库打不开时显示可读错误 | ✅ | `service::tests::a_failing_database_open_yields_a_readable_message` 断言错误信息含"书库";GUI 状态栏红色显示。查询期错误(如库被占用)也会显示在状态栏与书列表页脚 |
| C1 | 书列表表格视图 | ⚠️ **代码就绪,未观察** | `ui::Table` + `uniform_list` 虚拟滚动,8 列。数据在后台线程加载 |
| C2 | 列定义对齐上游 | ✅ | `book_list::tests::default_columns_hide_upstream_hidden_fields` 断言默认隐藏 `last_modified/languages/formats/id/path/pages`;上游 `gui2/library/views.py:1106` |
| C3 | 点击表头排序 | ⚠️ **代码就绪,未观察** | `book_list::tests::title_column_maps_to_title_sort_field` 等。**首版因缺 `cx.notify()` 导致排序不刷新**(审查发现),已修,待人工确认 |
| C4 | 大库分块加载 | ✅ | `book_list::tests::chunk_offsets_cover_the_whole_result_set` 覆盖 10 万本 / 50 块;加载在后台线程另开连接,不在 UI 线程全表扫描 |
| C5 | 非 ASCII / emoji / 超长书名 | ✅ | `book_list::tests::row_cells_render_cjk_emoji_and_long_titles` 等 3 项 |
| C6 | 视图状态持久化 + clamp | ✅ | `view_state::` 13 项测试;端到端:预置 2000px 面板宽 → 启动后 clamp 到 480+480(=1280 − 320)并回写;列宽 −50→24、99999→1200。**窗口缩小后放大会恢复原宽度**(收敛只作用于渲染副本,不写回) |
| C7 | 架构边界(只有 service.rs 依赖 caliberate) | ✅ | `architecture_tests::only_service_module_may_touch_backend_crates` —— 全文件扫描 + 运行时拼接目标串,`pub use` 与内联全路径都逃不掉 |
| C8 | 多选 | ⚠️ **代码就绪,未观察** | 纯逻辑 `selection_after_click` 有 4 项测试(普通点击 / Cmd 反选 / Shift 连选 / 反向连选);行点击处理已接上 |
| C9 | 不改写 calibre 的书库 | ✅ | `service::tests::refuses_to_open_a_calibre_library` 断言打开 calibre 的 `metadata.db` 被拒绝,且文件字节前后完全一致 |

## 2. 验证命令与结果

```
$ cargo fmt --check
(无输出 = 通过)

$ cargo clippy --all-targets --target aarch64-apple-darwin -- -D warnings
Finished `dev` profile —— 零 error 零 warning(本 crate)

$ cargo test --target aarch64-apple-darwin
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo build --target aarch64-apple-darwin
Finished `dev` profile —— 二进制 calibre4rust 生成成功
```

## 2.1 整分支审查发现与修复(2026-10-21)

交付前做了一次独立的整分支代码审查(`claude-opus-5.5` 之外的可用模型 `glm-5.3-prime`),
发现 **2 个 Critical + 7 个 Important**。全部已修,修复方式见 §7 与 ledger。

**最重要的一条(C1)**:首版所有交互式状态变更都通过 `Entity::update` 完成,而 GPUI 的
`Entity::update` **不会**触发 `cx.notify()` —— 拖分隔条、切换布局这些操作在视觉上
完全无效。原验收记录把它们标成了 ✅,那是**基于代码存在与常量断言,而不是实际观察**。
现已全部补上 `cx.notify()`,但**修复后的行为同样需要人工确认**(见 §4)。

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
2. **交互行为(拖拽 / 布局切换 / 排序 / 多选)** —— 整分支审查发现首版因缺 `cx.notify()` 而完全失效,现已修复并补了纯逻辑测试,但**修复后的行为仍未被肉眼观察**。这是当前最重要的开放项。
3. **拖拽分隔条的手感** —— 10px/50px 阈值与宽/窄布局切换的逻辑有测试覆盖,但交互手感需人工判断。
4. **超大书库的实际滚动性能** —— 分块逻辑有测试(10 万本 / 50 块),加载已移到后台线程;但真实滚动帧率未测。
5. **caliberate 在 Linux / Windows 的兼容性** —— 本项目只保证 macOS;caliberate 侧官方保证 Windows/Linux。

## 4.1 本轮审查中被"搁置"的行为(留给后续 Phase)

| 行为 | 理由 |
|---|---|
| 搜索查询鲁棒性(未转义引号/正则元字符/括号不配对) | 搜索栏是 Phase E 的占位;失效模式尚不存在。已登记为 Phase E 计划的验收项 |
| 损坏/DRM 书籍导入行为 | 本轮未实现导入;Phase I 计划时登记 |
| 文件名清理 / 书籍文件路径穿越 | 本轮没有写书籍文件的代码路径;Phase I 实现导入时重新检查 |
| CJK/重音不敏感的**搜索**匹配 | 搜索未实现(Phase E);排序交给 caliberate 的 SQL `ORDER BY` |
| 工具栏动作行为(捐赠、商店等) | 按计划故意保留占位符 |
| 无系列序号的系列书显示 `[1]` | caliberate schema 的默认值(后端外观问题) |
| Linux / Windows | 超出范围(macOS 优先) |

## 5. 与 spec 的偏离

| 项 | spec | 实际 | 理由 |
|---|---|---|---|
| `LibraryService::open` | `open(config: &ControlPlane)` | `open_path(path, busy_timeout_ms)` + `open_in_memory()` | `ControlPlane` 不派生 `Default` 且有 19 个必填字段,测试里构造要 150 行;control-plane TOML 接入留待后续 |
| service 的类型转发 | — | `service.rs` 转发 `LibraryBookSummary`/`LibraryQuery`/`LibrarySortField` 等纯数据类型 | 否则 UI 直接 import caliberate 会撞架构测试;边界只管查询路径,不管数据形状 |
| 额外 CLI | 计划放在 Task 6/21 | Task 4 就加了 `calibre4rust list` | 没有它就无法为"表格真的读出了数据"提供可观察证据 |
| 状态文件位置 | 用 `ControlPlane.paths` 的 GUI 状态目录 | `<library_dir>/view-state.json` | control-plane TOML 尚未接入 |
| GPUI 版本 | crates.io 0.2.2 | git rev `92447a17` | crates.io 版本是旧 API(`Application::new`),与当前 zed main 不兼容 |
| 几何收敛 | spec 未细说 | 收敛只作用于**渲染副本**,不写回持久化状态 | 整分支审查发现:写回会让"窗口缩小再放大"永久清零用户的面板宽度 |
| 搜索 | spec 计划了字段定向 | 本轮只做数据层,搜索栏仍是占位 | 搜索语法受 caliberate 缺口 #1 限制,留给 Phase E |

## 6. 后续阶段(各自另出计划)

| 阶段 | 内容 | 阻塞项 |
|---|---|---|
| D/E | 分类浏览器 + 搜索栏 | 分类浏览器可做(6 类 facet);搜索受 caliberate 缺口 #1 限制,只能做自由文本 + 字段定向 |
| F/G | 书籍详情 + 封面网格 | 封面取法待确认(缺口 #6) |
| H | 元数据编辑 | 需确认 `update_book_*` 是否支持多值(缺口 #5) |
| I | 工具栏动作 + 首选项 | — |
| J | 阅读器 | **阻塞于 caliberate 侧 `crates/document` 与 `crates/formats`**(缺口 #7) |
