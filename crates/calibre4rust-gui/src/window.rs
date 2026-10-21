use gpui::prelude::*;
use gpui::{App, Entity, Menu, MenuItem, MouseButton};
use ui::prelude::*;

use crate::actions;
use crate::service::LibraryService;

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

/// 拖过分隔条小于该像素宽度时隐藏面板。上游 `calibre/src/calibre/gui2/central.py:39-40`。
pub const PANEL_HIDE_THRESHOLD: f32 = 10.0;

/// 拖过分隔条大于该像素宽度时恢复显示面板。同上。
pub const PANEL_SHOW_THRESHOLD: f32 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLayout {
    Wide,
    Narrow,
}

/// 拖动分隔条时的起点状态。
#[derive(Debug, Clone, Copy)]
struct ResizeDrag {
    start_width: f32,
    start_x: f32,
}

pub struct MainWindow {
    service: Option<Entity<LibraryService>>,
    layout: PanelLayout,
    tag_browser_visible: bool,
    book_details_visible: bool,
    cover_browser_visible: bool,
    quick_view_visible: bool,
    left_panel_width: f32,
    right_panel_width: f32,
    left_resize: Option<ResizeDrag>,
    right_resize: Option<ResizeDrag>,
    status_message: String,
    service_error: Option<String>,
}

impl MainWindow {
    pub fn new(
        service: Option<Entity<LibraryService>>,
        service_error: Option<String>,
        cx: &mut App,
    ) -> Entity<Self> {
        let status_message = match service.as_ref() {
            Some(service) => match service.read(cx).book_count() {
                Ok(count) => format!("{count} 本书"),
                Err(error) => {
                    tracing::error!(%error, "统计书库书籍数失败");
                    "书库统计失败".to_string()
                }
            },
            None => "书库未加载".to_string(),
        };

        let window = cx.new(|_cx| Self {
            service,
            layout: PanelLayout::Wide,
            tag_browser_visible: true,
            book_details_visible: true,
            cover_browser_visible: false,
            quick_view_visible: false,
            left_panel_width: 280.0,
            right_panel_width: 360.0,
            left_resize: None,
            right_resize: None,
            status_message,
            service_error,
        });

        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleLayout, cx| {
                window.update(cx, |this, _cx| {
                    this.layout = match this.layout {
                        PanelLayout::Wide => PanelLayout::Narrow,
                        PanelLayout::Narrow => PanelLayout::Wide,
                    };
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleBookDetails, cx| {
                window.update(cx, |this, _cx| {
                    this.book_details_visible = !this.book_details_visible;
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleTagBrowser, cx| {
                window.update(cx, |this, _cx| {
                    this.tag_browser_visible = !this.tag_browser_visible;
                });
            }
        });
        cx.on_action({
            let window = window.clone();
            move |_: &actions::ToggleCoverGrid, cx| {
                window.update(cx, |this, _cx| {
                    this.cover_browser_visible = !this.cover_browser_visible;
                });
            }
        });

        window
    }

    pub fn layout(&self) -> PanelLayout {
        self.layout
    }

    /// 书库服务。书库不可用时为 `None`。
    pub fn service(&self) -> Option<Entity<LibraryService>> {
        self.service.clone()
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
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
                    _ => MenuItem::action(name, actions::ShowHelp),
                };
                Menu::new(name).items([item])
            })
            .collect()
    }
}

impl Render for MainWindow {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let status = self.status_message.clone();
        let service_error = self.service_error.clone();

        div()
            .flex_col()
            .size_full()
            .child(self.render_toolbar())
            .child(self.render_search_bar())
            .child(self.render_central_panels(cx))
            .child(
                div()
                    .h(px(24.0))
                    .px_2()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(gpui::rgba(0x00000018))
                    .child(status)
                    .when_some(service_error, |this, error| {
                        this.child(div().text_color(gpui::rgba(0xE5484D)).child(error))
                    }),
            )
    }
}

impl MainWindow {
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

    fn render_central_panels(&mut self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let narrow = self.layout == PanelLayout::Narrow;
        let left_width = if narrow {
            self.left_panel_width * 0.25
        } else {
            self.left_panel_width
        };
        let right_width = if narrow {
            self.right_panel_width * 0.35
        } else {
            self.right_panel_width
        };

        div()
            .flex_1()
            .flex_row()
            .overflow_hidden()
            .child(self.render_tag_browser(left_width))
            .child(self.render_left_handle(cx))
            .child(
                div()
                    .flex_1()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_cover_browser())
                    .child(self.render_horizontal_handle())
                    .child(self.render_book_list())
                    .child(self.render_horizontal_handle())
                    .child(self.render_quick_view()),
            )
            .child(self.render_right_handle(cx))
            .child(self.render_book_details(right_width))
    }

    fn render_tag_browser(&self, width: f32) -> impl IntoElement {
        if !self.tag_browser_visible {
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
        div().flex_1().h_full().p_2().child("书列表")
    }

    fn render_book_details(&self, width: f32) -> impl IntoElement {
        if !self.book_details_visible {
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

    fn render_cover_browser(&self) -> impl IntoElement {
        if !self.cover_browser_visible {
            return div().h_0();
        }
        div().h(px(160.0)).border_b_1().p_2().child("封面浏览器")
    }

    fn render_quick_view(&self) -> impl IntoElement {
        if !self.quick_view_visible {
            return div().h_0();
        }
        div().h(px(120.0)).border_t_1().p_2().child("快速预览")
    }

    fn render_horizontal_handle(&self) -> impl IntoElement {
        div()
            .w_full()
            .h(px(4.0))
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .bg(gpui::rgba(0x00000010))
    }

    fn render_left_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        let window_for_down = window.clone();
        let window_for_move = window.clone();
        let window_for_up = window.clone();
        div()
            .w(px(4.0))
            .h_full()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                window_for_down.update(cx, |this, _cx| {
                    this.left_resize = Some(ResizeDrag {
                        start_width: this.left_panel_width,
                        start_x: f32::from(event.position.x),
                    });
                });
            })
            .on_mouse_move(move |event, _window, cx| {
                window_for_move.update(cx, |this, _cx| {
                    let Some(drag) = this.left_resize else {
                        return;
                    };
                    let width =
                        (drag.start_width + (f32::from(event.position.x) - drag.start_x)).max(0.0);
                    this.left_panel_width = width;
                    if width < PANEL_HIDE_THRESHOLD {
                        this.tag_browser_visible = false;
                    } else if width > PANEL_SHOW_THRESHOLD {
                        this.tag_browser_visible = true;
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                window_for_up.update(cx, |this, _cx| this.left_resize = None);
            })
    }

    fn render_right_handle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let window = cx.entity();
        let window_for_down = window.clone();
        let window_for_move = window.clone();
        let window_for_up = window.clone();
        div()
            .w(px(4.0))
            .h_full()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(gpui::rgba(0x00000010))
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                window_for_down.update(cx, |this, _cx| {
                    this.right_resize = Some(ResizeDrag {
                        start_width: this.right_panel_width,
                        start_x: f32::from(event.position.x),
                    });
                });
            })
            .on_mouse_move(move |event, _window, cx| {
                window_for_move.update(cx, |this, _cx| {
                    let Some(drag) = this.right_resize else {
                        return;
                    };
                    let width =
                        (drag.start_width - (f32::from(event.position.x) - drag.start_x)).max(0.0);
                    this.right_panel_width = width;
                    if width < PANEL_HIDE_THRESHOLD {
                        this.book_details_visible = false;
                    } else if width > PANEL_SHOW_THRESHOLD {
                        this.book_details_visible = true;
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                window_for_up.update(cx, |this, _cx| this.right_resize = None);
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
        assert_eq!(PANEL_HIDE_THRESHOLD, 10.0);
        assert_eq!(PANEL_SHOW_THRESHOLD, 50.0);
    }

    #[test]
    fn every_menubar_entry_becomes_a_top_level_menu() {
        let menus = MainWindow::native_menus();
        assert_eq!(menus.len(), menubar_actions().len());
    }
}
