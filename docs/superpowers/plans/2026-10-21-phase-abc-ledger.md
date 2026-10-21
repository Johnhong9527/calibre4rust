# SDD ledger — plan: docs/superpowers/plans/2026-10-21-phase-abc-gpui-frontend.md

分支:`feat/phase-abc-gpui-frontend`
仓库:`/Users/sysadmin/Desktop/code/wb_projec/zed2calibre_project/calibre4rust`
Spec:`docs/superpowers/specs/2026-10-21-calibre4rust-design.md`

## 环境事实(2026-10-21 实测)

- 本机 macOS aarch64;Rust 1.98.1 需用 USTC 镜像安装(直连超时)
- crates.io 必须走 rsproxy 镜像(直连 10 分钟卡在索引更新;镜像后 1m39s)
- 禁用 `cargo metadata`(会解析所有 target,>15 分钟未完成)
- caliberate 全 14 crate 在 macOS `cargo check --workspace --all-targets` 零错误通过
- GPUI git 依赖 + `runtime_shaders` 实测可编译运行;**像素渲染未肉眼确认**(本机无录屏权限)

## Pre-flight:共享接口扫描

| 生产方 | 消费方 | 接口 | 结论 |
|---|---|---|---|
| T1 | T2 | crate `calibre4rust_gui`;`app.rs`/`lib.rs` 由 T2 追加 `pub mod service;` | 一致 |
| T1 | T3 | `app::run()` 开窗;T3 改为开 `MainWindow` | 一致 |
| T2 | T3 | `LibraryService` → `MainWindow::new(service: Entity<LibraryService>, cx)` | 一致 |
| T2 | T4 | `LibraryService` + `default_library_query()` + `SUMMARY_CHUNK_SIZE` | 一致 |
| T3 | T4 | `window.rs` 的书列表占位面板 → T4 替换为真实表格 | 一致 |
| T4 | T5 | `book_list.rs` 由 T5 接入 `ViewState`(列宽/列可见性/排序) | 一致 |
| T2 | T6 | `docs/caliberate-gaps.md` 由 T6 汇总 | 一致 |

**Pre-flight:无其它共享接口。**

## Pre-flight:自洽性扫描

| # | 项 | 发现 | 裁定 |
|---|---|---|---|
| 1 | T2 的架构边界测试 | 计划里用 `contents.contains("use caliberate_")` 全文件子串扫描 —— 但测试代码本身写在 `lib.rs` 里,其字符串字面量 `"caliberate_db::"` 会被 `contains` 命中,导致 `lib.rs` 被误判为违规者,测试必失败 | **Ruling: 改为按行扫描 `line.trim_start().starts_with("use calibre")`**,字面量所在行不以 `use` 开头,自然排除 —— 理由:保留"只有 service.rs 能 import caliberate crate"的强制力,又不自伤。代价:若有人把 import 写成 `pub use calibre...` 会被 `starts_with("use calibre")` 漏掉 → 改为同时匹配 `use calibre` 与 `pub use calibre` |
| 2 | T2 `LibraryService::open(&ControlPlane)` | 测试需要构造 `ControlPlane`;caliberate 的 `ControlPlane` 是否 `Default` 派生、字段是否 `pub` 未知 | 实现时读 `../caliberate/crates/core/src/config.rs` 确认;若不可 `Default` 则在测试里逐字段构造 |
| 3 | T4 `format_row_cell` | `LibraryBookSummary.series` 的确切类型(`Option<LibrarySeriesSummary>`,含 `name` / `index: Option<f64>`)未核实 | 实现时读 `../caliberate/crates/library/src/summary.rs` 确认 |
| 4 | T2/T4 `LibraryQuery` | 是否实现 `Default` 未核实 | 同上,读 `../caliberate/crates/library/src/query.rs`;若无 `Default` 则逐字段构造 |
| 5 | `Entity<LibraryService>` | GPUI 的 `cx.new()` 是否要求 `T: Render` 未核实 | 实现时确认;若要求,则给 `LibraryService` 实现 `Render`(返回空元素)或改用 `cx.new` 的其他形式 |

## Rulings

- Pre-flight #1:架构测试改为按行扫描(`use calibre` / `pub use calibre` 前缀),不用全文件 `contains` —— 理由:测试自身的字符串字面量会自伤 —— 代价:漏检把 import 藏在宏/别名里的写法,风险可接受

## Task 1

Task 1: Ruling: caliberate 的 git 依赖去掉 `path` 键 —— cargo 报 *"dependency (caliberate-core) specification is ambiguous. Only one of `git` or `path` is allowed."*;不写 path 时 cargo 在 checkout 的 workspace members 里按包名查找。代价:若某个包名在 caliberate workspace 中不唯一,解析会失败(已验证唯一)
Task 1: Ruling: build.rs 改为先把 SVG 复制到 `OUT_DIR/icons/` 再用 `include_bytes!(concat!(env!("OUT_DIR"), ...))` 引用 —— 计划里的 `include_bytes!("../../assets/icons/...")` 会被解析到生成文件所在的 OUT_DIR(而非 crate 根),且该路径在包目录之外。代价:OUT_DIR 多一份约 2 MB 的图标副本
Task 1: Ruling: 测试命令用 `-p calibre4rust-gui`(连字符,包名)而非 `-p calibre4rust_gui`(lib 名)—— cargo 的 `-p` 匹配包名。代价:无
Task 1: Ruling: `Application::run` 的回调只能返回 `()`,开窗失败用 `Rc<RefCell<Option<anyhow::Error>>>` 带出后转成 `Err` —— 不用 `unwrap()`/`expect()`(AGENTS.md 禁止),也不静默吞掉。代价:多一层 Rc
Task 1: Ruling: `ui::Button::new` 需 `(id, label)` 两个参数;`on_click` 来自 `ui::traits::clickable::Clickable`,经 `ui::prelude::*` 引入 —— 计划里的 `ui::Button::new("点击我")` 与缺 trait 导入均编译不过。代价:无
Task 1: Ruling: 新增 `rustfmt.toml`(edition/style_edition 2024)与 zed 对齐 —— 计划未提,但 AGENTS.md 要求沿用 zed 的 Rust 规范。代价:无
Task 1: Ruling: 肉眼验证无法完成 —— 本机无录屏/辅助功能权限,`screencapture` 与 `osascript` 均被拒。已验证替代证据:进程启动后存活、事件循环运行(RSS 108 MB,CPU 1.1%)、无任何 stderr 输出、无 Metal/shader 错误。**像素渲染仍未肉眼确认**,需在有录屏权限的机器上补做。代价:Phase A 验收有一项待人工确认
Task 1: complete (commits 5fc2b7d..7b0a1b4, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' -p calibre4rust-gui --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## Task 2

Task 2: Ruling: `LibraryService::open` 的签名从 `open(config: &ControlPlane)` 改为 `open_path(path: &Path, busy_timeout_ms: u64)` —— `ControlPlane` 不派生 `Default` 且有 19 个必填字段,测试里构造它要写约 150 行;caliberate 的 `Database::open_path` 直接接受路径。代价:app 层将来要自己从 `ControlPlane` 取 `db`/`fts` 两项来调用
Task 2: Ruling: `LibraryService::open_path` 在开库前 `create_dir_all` 父目录 —— rusqlite 的 `Connection::open` 不建父目录,缺这一步"打开不存在的书库"会失败。代价:无
Task 2: Ruling: 架构边界测试用按行扫描(`use calibre` / `pub use calibre` / `use ::calibre` 前缀)而非全文件 `contains` —— 见 Pre-flight #1,测试自身的字符串字面量会自伤。代价:把 import 藏在宏或别名里会漏检
Task 2: Ruling: 窗口的书数存放在 `RootView` 自身字段,`LibraryService` 保持只读(不引入 `Cell`/`AtomicUsize` 缓存) —— 避免为 Task 2 引入不必要的内部可变性;刷新时机在 Task 4 随书列表一起做。代价:书数在书库外部变化时不会自动更新
Task 2: Ruling: `Entity<LibraryService>` 可用 —— `Database` 内含 `rusqlite::Connection`(Send 但非 Sync),gpui 的 `Entity<T>` 无 `Send`/`Sync` 约束,实测编译通过。已验证:Task 4 若需后台加载需另行确认
Task 2: Ruling: 库的默认位置用 `CALIBRE4RUST_LIBRARY` 环境变量覆盖,否则 `~/Library/Application Support/calibre4rust/library/calibre.db` —— control-plane TOML 的接入留到后续阶段(caliberate 的 `ControlPlane` 需要完整配置文件)。代价:暂不支持 TOML 配置
Task 2: complete (commits 7b0a1b4..80b7385, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' -p calibre4rust-gui --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## Task 3

Task 3: Ruling: calibre 的 macOS 原生菜单栏按"每项一个顶级菜单"实现 —— 上游 `gui2/bars.py` 的 `MenuBar.init_bar` 对列表里每一项调 `mb.addAction(..., is_top_level=True)`,不是嵌套子菜单。代价:每个顶级菜单里暂时只有同名的一个菜单项
Task 3: Ruling: 分隔条用 `on_mouse_down`/`on_mouse_move`/`on_mouse_up` 三件套而非 `on_drag` —— `on_drag` 需要构造一个 `Entity<W: Render>` 作为拖拽幽灵,对面板缩放是额外负担;三件套直接改宽度。代价:拖拽时没有系统级拖拽光标反馈(已用 `.cursor(CursorStyle::ResizeLeftRight)` 补偿)
Task 3: Ruling: 阈值判定放在拖拽过程中(宽度 <10 隐藏、>50 恢复),而非只在拖拽结束时 —— 对齐上游 `central.py` 的交互手感。代价:无
Task 3: Ruling: `MainWindow` 的 service 字段是 `Option<Entity<LibraryService>>` —— 书库打不开时用内存库兜底,连内存库都失败时为 `None`,避免 `unwrap`/`unreachable` 之类的 panic。代价:调用方每次要处理 `None`
Task 3: Ruling: `LibraryService::open_in_memory()` 用 `Database::open_path(":memory:", ...)` —— rusqlite 支持内存库,可建出带完整 schema 的空库作为占位。代价:内存库每次启动都要跑一遍 migrate
Task 3: Ruling: 状态栏的书数在窗口创建时算一次并存入 `status_message` —— 每帧读数据库会卡 UI;真正的刷新时机在 Task 4 随书列表一起做。代价:书库外部变化后书数不自动更新
Task 3: Ruling: `MainWindow::service()` 访问器存在,只为让字段被读(消除 dead_code)且供 Task 4 使用 —— 不加 `#[allow(dead_code)]` 压制。代价:无
Task 3: complete (commits 80b7385..928f70a, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' -p calibre4rust-gui --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## Task 4

Task 4: Ruling: 纯数据类型(`LibraryBookSummary`/`LibraryQuery`/`LibrarySortField`)由 `service.rs` 转发,UI 模块从 `crate::service` 取 —— 否则 book_list.rs 直接 import `caliberate_library` 会撞上"只有 service.rs 能依赖 caliberate"的架构测试。架构边界关心的是**查询路径**(catalog/database/SQL),不是数据形状。代价:service.rs 的 re-export 面会随 Phase 增长
Task 4: Ruling: 新增 `calibre4rust list` CLI 子命令 —— 计划把 CLI 放在 Task 6/21,但没有它就无法为"表格真的读出了数据"提供可观察的输出证据;caliberate 自己也把 CLI 当一等公民。代价:CLI 面提前了一点
Task 4: Ruling: 排序只做单级 —— `LibraryQuery` 只支持单级排序(见 docs/caliberate-gaps.md 缺口 #3);再次点击同一列切换升降序。代价:与 calibre 的多级排序不等价
Task 4: Ruling: 分块加载先取 `page.total` 再按 2000 一块拉 —— 避免 `list_books()` 在大库上一次全表加载;照搬 caliberate egui 侧的分块模式(`crates/gui/src/views.rs:84-101`)。代价:多一次查询
Task 4: Ruling: 表头排序通过 `.id(...)` 把 `Div` 转成 `Stateful<Div>` —— gpui 的 `on_click` 在 `StatefulInteractiveElement` 上,`Div` 只实现 `InteractiveElement`。代价:每个表头单元格带一个 id
Task 4: Ruling: 评分单元格除以 2 —— calibre 内部评分存 0–10(2×星),显示时转成星。代价:无
Task 4: Ruling: `LibraryFormat` 的字段是 `format`/`size_bytes`(不是 `name`)—— 按实际定义对齐。代价:无
Task 4: Ruling: 端到端验证用 caliberate 自己的 `calibredb add` 写入、我们的 `calibre4rust list` 读出 —— 验证的是真实的数据链路,不是造数据。已在 `/tmp/c4r-e2e` 用两份手工构造的 EPUB(含中文、emoji、NCX 目录)验证通过。代价:验证脚本是一次性的,未入库
Task 4: complete (commits 928f70a..7edbad6, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' -p calibre4rust-gui --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## Task 5

Task 5: Ruling: `ViewState` 实现 `Default` trait(而不是自定义 inherent `default()`),并给每个字段配 `#[serde(default = "...")]` —— serde 的字段级 `default` 用的是**字段类型**的 Default(`f32` → 0.0),不是结构体的 Default;不配 per-field 默认函数,读到缺字段的 JSON 会得到 0 宽的面板。代价:多几个 default 函数
Task 5: Ruling: 状态文件放在书库数据库同目录(`<library_dir>/view-state.json`) —— control-plane TOML 尚未接入(spec §9 记为待办),先用书库目录就近存放。代价:换书库时状态不跟随
Task 5: Ruling: 状态在每次渲染时与书列表的排序同步,几何按当前窗口尺寸 clamp,变了才落盘 —— GPUI 92447a1 没有 `WindowEvent`,拿不到可靠的"窗口关闭"回调;每帧比对整个结构体开销可忽略。代价:崩溃时最后一次状态可能没落盘
Task 5: Ruling: 面板宽度的 clamp 是"先各自夹到可用宽度,再按比例缩放" —— 保证 left+right <= 窗口宽 − 最小书列表宽,且操作幂等(测试覆盖)。代价:两个面板同时超宽时按比例缩放,可能不是用户直觉的比例
Task 5: Ruling: 窗口尺寸用 `window.viewport_size()` —— gpuui 的 `Window::viewport_size()`。代价:无
Task 5: complete (commits 7edbad6..d8d8e31, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' -p calibre4rust-gui --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## Task 6

Task 6: Ruling: 门禁额外加了 release 构建 —— 计划只要求 debug 构建;release 才能暴露只在优化下出现的 UB/溢出,也能确认二进制可分发。实测 2m14s 通过,二进制 20 MB。代价:多花约 2 分钟构建时间
Task 6: Ruling: 验收记录与 gaps 文档入库 —— 它们是 Phase A/B/C 的交付证据,不能只活在会话里。代价:无
Task 6: complete (commits d8d8e31..bdfc8b1, tests: cargo test --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' --target aarch64-apple-darwin → test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s)

## 整分支审查(2026-10-21)

审查模型:`openrouter/z-ai/glm-5.3-prime`(本区域可用的最强模型;`claude-opus-5.5` 与
`gpt-6.1-sol-pro` 均报 "not available in your region")。
审查包:`.superpowers/sdd/2026-10-21-phase-abc-gpui-frontend/review-5fc2b7d..bdfc8b1.diff`

结论:**2 Critical + 7 Important + 若干 Minor**。评审判定 "Ready to merge: No"。

### 已修(Critical / Important 全部)

Final: fixed C1 交互式状态变更从不调 cx.notify() —— 拖分隔条/切布局/排序视觉上完全无效 —— 全部变更处理器补 cx.notify();测试 `view_state::tests::toggling_layout_alternates_between_wide_and_narrow` 等 13 项覆盖拖拽与切换的纯逻辑,suite 41/41
Final: fixed C2 分隔条拖动无指针捕获 —— 改为在 render 里每帧注册窗口级 MouseMoveEvent/MouseUpEvent(gpui 的 Window::on_mouse_event 只活一帧,必须在 render 里注册);测试 `view_state::tests::dragging_the_left_panel_below_the_threshold_hides_it` RED→GREEN,suite 41/41
Final: fixed I1 全量加载在 UI 线程 + 每次渲染克隆整份数据集 —— 计数改 page.total、加载移到后台线程(另开只读连接)、渲染按范围读取;测试 `service::tests::opening_a_missing_library_directory_creates_it` 与 `book_list::tests::chunk_offsets_cover_the_whole_result_set` 覆盖,suite 41/41
Final: fixed I2 多选完全缺失 —— 实现 selection_after_click(普通/Cmd 反选/Shift 连选/反向连选),4 项新测试,suite 41/41
Final: fixed I3 横向分隔条是装饰 —— 封面浏览器与快速预览高度可拖;测试 `view_state::tests::dragging_the_cover_browser_shrinks_it_when_moving_down` RED→GREEN,suite 41/41
Final: fixed I4 几何收敛只缩小且被持久化,窗口缩小再放大永久丢面板宽度 —— 收敛改为只作用于渲染副本;测试 `view_state::tests::clamping_does_not_destroy_the_saved_layout` RED→GREEN,suite 41/41
Final: fixed I5 架构测试可被 pub use / 内联全路径绕过 —— 改为全文件扫描 + 运行时拼接目标串(caliberate_ 前缀,避开散文误报);测试 `architecture_tests::only_service_module_may_touch_backend_crates` 自身在实现前会失败,suite 41/41
Final: fixed I6 打开任何 SQLite 文件都会跑 schema 迁移,指向 calibre 的 metadata.db 会改写用户数据 —— 打开前检测 calibre 特征并拒绝;测试 `service::tests::refuses_to_open_a_calibre_library` 断言文件字节前后一致,端到端 sha256 前后一致,suite 41/41
Final: fixed I7 查询期错误对用户不可见 —— 存到 BookList::last_error,状态栏与书列表页脚显示

### 已修(Minor 中成本低的)

Final: fixed build.rs 的 rerun-if-changed 不盯子目录(icons/plugins 新增文件不触发重建)
Final: fixed HOME 缺失时回落到 "/"(会尝试写 /Library/...)—— 改落到临时目录
Final: fixed 状态文件解析失败被 unwrap_or_default 静默吞掉 —— 补 tracing::warn
Final: fixed 书数在主窗口状态栏与书列表页脚重复显示
Final: fixed 隐藏列已接入 BookList::apply_hidden_columns(UI 开关仍缺,记为 F5)

### 审查者"拒绝评判"的条目(已裁决:留给后续 Phase)

Final: Ruling: 搜索查询鲁棒性(未转义引号/正则元字符/括号不配对)—— 留给 Phase E 计划作为明确验收项,本轮搜索栏尚未实现,失效模式不存在。代价:Phase E 必须补上
Final: Ruling: 损坏/DRM 书籍导入行为 —— Phase I 实现导入时登记并覆盖。代价:Phase I 需补
Final: Ruling: 文件名清理 / 书籍文件路径穿越 —— 本轮没有写书籍文件的代码路径,Phase I 实现导入时重新检查。代价:Phase I 需补
Final: Ruling: CJK/重音不敏感的搜索匹配 —— 搜索未实现(Phase E);排序交给 caliberate 的 SQL ORDER BY(后端行为)。代价:Phase E 需补
Final: Ruling: 工具栏动作行为(捐赠、商店等)—— 按计划故意保留占位符。代价:Phase I
Final: Ruling: 无系列序号的系列书显示 [1] —— caliberate schema 默认值,后端外观问题,不属前端。代价:无
Final: Ruling: Linux / Windows —— 超出范围(macOS 优先)。代价:跨平台支持推迟
Final: Ruling: 像素级渲染与交互手感 —— 需要有录屏权限的环境,已在验收记录 §4 列为最重要的开放项。代价:Phase A/B/C 有一项验收未闭合
Final: Ruling: 10 万本真实帧率 —— 无此规模的书库可测,按结构评估(加载已移出 UI 线程)。代价:真实性能未测
