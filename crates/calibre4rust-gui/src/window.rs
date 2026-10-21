use gpui::prelude::*;
use gpui::{App, DispatchPhase, Entity, Menu, MenuItem, MouseButton, MouseMoveEvent, MouseUpEvent};
use std::path::PathBuf;
use ui::prelude::*;

use crate::actions;
use crate::book_list::BookList;
use crate::service::LibraryService;
use crate::view_state::ViewState;

/// calibre 的 macOS 默认工具栏动作。
///
/// 上游依据 `calibre/src/calibre/gui2/__init__.py:359-376` —— macOS 分支**不含**
/// Help 与 Preferences(它们在原生菜单栏里)。非 macOS 的默认布局在 `:399-417`。
pub fn toolbar_actions() -> Vec<&'static str> {
    vec![
        "Add Books",
        "Edit Metadata",
        "---",
        "Convert Books",
        "View",
        "---",
        "Choose Library",
        "Donate",
        "---",
        "Fetch News",
        "Store",
        "Save To Disk",
        "Connect Share",
        "---",
        "Remove Books",
        "Tweak ePub",
    ]
}

/// calibre 的原生菜单栏动作。
///
/// 上游依据 `calibre/src/calibre/gui2/__init__.py:331-340`。calibre 在 macOS 上把
/// 列表里的每一项做成一个**顶级菜单**(`gui2/bars.py` 的 `MenuBar.init_bar`)。
pub fn menubar_actions() -> Vec<&'static str> {
    vec![
        "Add Books",
        "Edit Metadata",
        "Convert Books",
        "Choose Library",
        "Save To Disk",
        "Preferences",
        "Help",
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLayout {
    Wide,
    Narrow,
}

/// 拖动分隔条时的起点状态。
#[derive(Debug, Clone, Copy)]
struct ResizeDrag {
    start_size: f32,
    start_pointer: f32,
}

pub struct MainWindow {
    service: Option<Entity<LibraryService>>,
    book_list: Entity<BookList>,
    view_state: ViewState,
    state_path: PathBuf,
    left_resize: Option<ResizeDrag>,
    right_resize: Option<ResizeDrag>,
    cover_resize: Option<ResizeDrag>,
    quick_resize: Option<ResizeDrag>,
    status_message: String,
    service_error: Option<String>,
}

impl MainWindow {
    pub fn new(
        service: Option<Entity<LibraryService>>,
        service_error: Option<String>,
        state_path: PathBuf,
        cx: &mut App,
    ) -> Entity<Self> {
        let view_state = ViewState::load(&state_path);

        let status_message = match service.as_ref() {
            Some(service) => match service.read(cx).book_total() {
                Ok(count) => format!("{count} 本书"),
                Err(error) => {
                    tracing::error!(%error, "统计书库书籍数失败");
                    "书库统计失败".to_string()
                }
            },
            None => "书库未加载".to_string(),
        };

        let book_list = BookList::new(service.clone(), cx);
        book_list.update(cx, |list, cx| {
            list.apply_hidden_columns(&view_state.hidden_columns);
            list.apply_sort(
                &view_state.selected_sort_column,
                view_state.sort_descending,
                cx,
            );
        });

        let window = cx.new(|_cx| Self {
            service,
            book_list,
            view_state,
            state_path,
            left_resize: None,
            right_resize: None,
            cover_resize: None,
            quick_resize: None,
            status_message,
            service_error,
        });

        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleLayout, cx| {
                window.update(cx, |this, cx| {
                    this.view_state.toggle_layout();
                    cx.notify();
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleBookDetails, cx| {
                window.update(cx, |this, cx| {
                    this.view_state.book_details_visible = !this.view_state.book_details_visible;
                    cx.notify();
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleTagBrowser, cx| {
                window.update(cx, |this, cx| {
                    this.view_state.tag_browser_visible = !this.view_state.tag_browser_visible;
                    cx.notify();
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleCoverGrid, cx| {
                window.update(cx, |this, cx| {
                    this.view_state.cover_browser_visible = !this.view_state.cover_browser_visible;
                    cx.notify();
                });
            }
        });

        window
    }

    pub fn layout(&self) -> PanelLayout {
        if self.view_state.is_narrow_layout() {
            PanelLayout::Narrow
        } else {
            PanelLayout::Wide
        }
    }

    /// 书库服务。书库不可用时为 `None`。
    pub fn service(&self) -> Option<Entity<LibraryService>> {
        self.service.clone()
    }

    /// 把 calibre 的菜单栏动作列表转成 GPUI 的原生菜单。
    ///
    /// calibre 在 macOS 上把每一项做成顶级菜单,这里保持一致。
    pub fn native_menus() -> Vec<Menu> {
        menubar_actions()
            .into_iter()
            .map(|name| {
                let item = match name {
                    "Add Books" => MenuItem::action(name, actions::AddBooks),
                    "Edit Metadata" => MenuItem::action(name, actions::EditMetadata),
                    "Convert Books" => MenuItem::action(name, actions::ConvertBooks),
                    "Choose Library" => MenuItem::action(name, actions::ChooseLibrary),
                    "Save To Disk" => MenuItem::action(name, actions::SaveToDisk),
                    "Preferences" => MenuItem::action(name, actions::OpenPreferences),
                    "Help" => MenuItem::action(name, actions::ShowHelp),
                    other => MenuItem::action(other, actions::ShowHelp),
                };
                Menu::new(name).items([item])
            })
            .collect()
    }
}

impl Render for MainWindow {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        // 渲染用的几何:按当前窗口收敛,但**不**写回持久化状态 —— 用户把窗口缩到
        // 很小再放大,面板宽度应该回到他原先设定的大小。
        let layout = self
            .view_state
            .clamped_for_window(f32::from(viewport.width), f32::from(viewport.height));

        self.sync_sort_state(cx);
        self.register_drag_listeners(window, cx);

        let status = self.status_message.clone();
        let service_error = self.service_error.clone();
        let list_error = self.book_list.read(cx).last_error().map(str::to_string);
        let selected_count = self.book_list.read(cx).selected_book_ids().len();

        div()
            .flex_col()
            .size_full()
            .child(self.render_toolbar())
            .child(self.render_search_bar())
            .child(self.render_central_panels(&layout, cx))
            .child(
                div()
                    .h(px(24.0))
                    .px_2()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(gpui::rgba(0x00000018))
                    .child(status)
                    .child(format!("已选 {selected_count}"))
                    .when_some(service_error, |this, error| {
                        this.child(div().text_color(gpui::rgba(0xE5484D)).child(error))
                    })
                    .when_some(list_error, |this, error| {
                        this.child(div().text_color(gpui::rgba(0xE5484D)).child(error))
                    }),
            )
    }
}

impl MainWindow {
    /// 把书列表的排序同步回状态;有变化就落盘。
    fn sync_sort_state(&mut self, cx: &mut gpui::Context<Self>) {
        let (sort_column, sort_descending) = self.book_list.read(cx).sort_state();
        if self.view_state.selected_sort_column == sort_column
            && self.view_state.sort_descending == sort_descending
        {
            return;
        }
        self.view_state.selected_sort_column = sort_column;
        self.view_state.sort_descending = sort_descending;
        if let Err(error) = self.view_state.save(&self.state_path) {
            tracing::error!(%error, "保存视图状态失败");
        }
    }

    /// 拖动期间在**窗口级**注册鼠标监听。
    ///
    /// GPUI 的 `Window::on_mouse_event` 只对当前帧有效(监听器存在 `next_frame` 里,
    /// 下一帧渲染时会被清掉),所以必须在每次 `render` 里重新注册。
    ///
    /// 有了窗口级监听:鼠标移出 4px 宽的把手也能继续拖,在别处松手也能正确结束拖动。
    fn register_drag_listeners(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        let dragging = self.left_resize.is_some()
            || self.right_resize.is_some()
            || self.cover_resize.is_some()
            || self.quick_resize.is_some();
        if !dragging {
            return;
        }

        let for_move = cx.entity();
        window.on_mouse_event::<MouseMoveEvent>(move |event, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            for_move.update(cx, |this, cx| {
                let x = f32::from(event.position.x);
                let y = f32::from(event.position.y);
                if let Some(drag) = this.left_resize {
                    this.view_state
                        .drag_left_panel_to(x, drag.start_size, drag.start_pointer);
                }
                if let Some(drag) = this.right_resize {
                    this.view_state
                        .drag_right_panel_to(x, drag.start_size, drag.start_pointer);
                }
                if let Some(drag) = this.cover_resize {
                    this.view_state
                        .drag_cover_browser_to(y, drag.start_size, drag.start_pointer);
                }
                if let Some(drag) = this.quick_resize {
                    this.view_state
                        .drag_quick_view_to(y, drag.start_size, drag.start_pointer);
                }
                cx.notify();
            });
        });

        let for_up = cx.entity();
        window.on_mouse_event::<MouseUpEvent>(move |_event, _phase, _window, cx| {
            for_up.update(cx, |this, cx| {
                this.left_resize = None;
                this.right_resize = None;
                this.cover_resize = None;
                this.quick_resize = None;
                if let Err(error) = this.view_state.save(&this.state_path) {
                    tracing::error!(%error, "保存视图状态失败");
                }
                cx.notify();
            });
        });
    }

    fn render_toolbar(&self) -> impl IntoElement {
        let mut row = div().h(px(36.0)).px_2().items_center().gap_2();
        for action in toolbar_actions() {
            row = if action == "---" {
                row.child(div().w(px(1.0)).h(px(20.0)).bg(gpui::rgba(0x00000020)))
            } else {
                row.child(
                    ui::Button::new(format!("toolbar-{action}"), action.to_string())
                        .size(ui::ButtonSize::Compact),
                )
            };
        }
        row
    }

    fn render_search_bar(&self) -> impl IntoElement {
        div()
            .h(px(40.0))
            .px_2()
            .items_center()
            .gap_2()
            .child(ui::Button::new("virtual-library", "虚拟库").size(ui::ButtonSize::Compact))
            .child(ui::Button::new("clear-virtual-library", "清除").size(ui::ButtonSize::Compact))
            .child(ui::Button::new("sort", "排序").size(ui::ButtonSize::Compact))
            .child(ui::Button::new("group-by", "分组").size(ui::ButtonSize::Compact))
            .child(
                ui::Button::new("search-box-placeholder", "搜索所有字段")
                    .size(ui::ButtonSize::Compact),
            )
            .child(ui::Button::new("search", "搜索").size(ui::ButtonSize::Compact))
    }

    fn render_central_panels(
        &mut self,
        layout: &ViewState,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let narrow = layout.is_narrow_layout();
        let left_width = if narrow {
            layout.left_panel_width * 0.25
        } else {
            layout.left_panel_width
        };
        let right_width = if narrow {
            layout.right_panel_width * 0.35
        } else {
            layout.right_panel_width
        };

        div()
            .flex_1()
            .flex_row()
            .overflow_hidden()
            .child(self.render_tag_browser(layout, left_width))
            .child(self.render_left_handle(cx))
            .child(
                div()
                    .flex_1()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_cover_browser(layout))
                    .child(self.render_cover_handle(cx))
                    .child(self.render_book_list())
                    .child(self.render_quick_handle(cx))
                    .child(self.render_quick_view(layout)),
            )
            .child(self.render_right_handle(cx))
            .child(self.render_book_details(layout, right_width))
    }

    fn render_tag_browser(&self, layout: &ViewState, width: f32) -> impl IntoElement {
        if !layout.tag_browser_visible {
            return div().w_0().child("分类浏览器");
        }
        div()
            .w(px(width))
            .h_full()
            .border_r_1()
            .border_color(gpui::rgba(0x00000018))
            .p_2()
            .child("分类浏览器")
    }

    fn render_book_list(&self) -> impl IntoElement {
        self.book_list.clone()
    }

    fn render_book_details(&self, layout: &ViewState, width: f32) -> impl IntoElement {
        if !layout.book_details_visible {
            return div().w_0().child("书籍详情");
        }
        div()
            .w(px(width))
            .h_full()
            .border_l_1()
            .border_color(gpui::rgba(0x00000018))
            .p_2()
            .child("书籍详情")
    }

    fn render_cover_browser(&self, layout: &ViewState) -> impl IntoElement {
        if !layout.cover_browser_visible {
            return div().h_0();
        }
        div()
            .h(px(layout.cover_browser_height))
            .border_b_1()
            .p_2()
            .child("封面浏览器")
    }

    fn render_quick_view(&self, layout: &ViewState) -> impl IntoElement {
        if !layout.quick_view_visible {
            return div().h_0();
        }
        div()
            .h(px(layout.quick_view_height))
            .border_t_1()
            .p_2()
            .child("快速预览")
    }

    fn render_cover_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        div()
            .w_full()
            .h(px(4.0))
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                let start_size = window.read(cx).view_state.cover_browser_height;
                window.update(cx, |this, cx| {
                    this.cover_resize = Some(ResizeDrag {
                        start_size,
                        start_pointer: f32::from(event.position.y),
                    });
                    cx.notify();
                });
            })
    }

    fn render_quick_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        div()
            .w_full()
            .h(px(4.0))
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                let start_size = window.read(cx).view_state.quick_view_height;
                window.update(cx, |this, cx| {
                    this.quick_resize = Some(ResizeDrag {
                        start_size,
                        start_pointer: f32::from(event.position.y),
                    });
                    cx.notify();
                });
            })
    }

    fn render_left_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        div()
            .w(px(4.0))
            .h_full()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                let start_size = window.read(cx).view_state.left_panel_width;
                window.update(cx, |this, cx| {
                    this.left_resize = Some(ResizeDrag {
                        start_size,
                        start_pointer: f32::from(event.position.x),
                    });
                    cx.notify();
                });
            })
    }

    fn render_right_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        div()
            .w(px(4.0))
            .h_full()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                let start_size = window.read(cx).view_state.right_panel_width;
                window.update(cx, |this, cx| {
                    this.right_resize = Some(ResizeDrag {
                        start_size,
                        start_pointer: f32::from(event.position.x),
                    });
                    cx.notify();
                });
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_toolbar_matches_upstream_calibre_default() {
        let actions = toolbar_actions();
        assert_eq!(actions[0], "Add Books");
        assert_eq!(actions[1], "Edit Metadata");
        assert!(!actions.contains(&"Help"), "macOS 工具栏不应有 Help");
        assert!(
            !actions.contains(&"Preferences"),
            "macOS 工具栏不应有 Preferences"
        );
        assert_eq!(
            actions,
            vec![
                "Add Books",
                "Edit Metadata",
                "---",
                "Convert Books",
                "View",
                "---",
                "Choose Library",
                "Donate",
                "---",
                "Fetch News",
                "Store",
                "Save To Disk",
                "Connect Share",
                "---",
                "Remove Books",
                "Tweak ePub",
            ]
        );
    }

    #[test]
    fn menubar_contains_preferences_and_help() {
        let actions = menubar_actions();
        assert_eq!(
            actions,
            vec![
                "Add Books",
                "Edit Metadata",
                "Convert Books",
                "Choose Library",
                "Save To Disk",
                "Preferences",
                "Help",
            ]
        );
    }

    #[test]
    fn panel_thresholds_match_upstream_calibre() {
        assert_eq!(
            crate::view_state::PANEL_HIDE_THRESHOLD,
            10.0,
            "上游 calibre gui2/central.py:39-40"
        );
        assert_eq!(crate::view_state::PANEL_SHOW_THRESHOLD, 50.0);
    }

    #[test]
    fn every_menubar_entry_becomes_a_top_level_menu() {
        let menus = MainWindow::native_menus();
        assert_eq!(menus.len(), menubar_actions().len());
    }
}
