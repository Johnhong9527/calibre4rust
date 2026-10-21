use gpui::{App, Bounds, Entity, WindowBounds, WindowOptions, size};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use theme::LoadThemes;
use ui::prelude::*;

use crate::assets::CalibreAssets;
use crate::service::LibraryService;
use crate::theme_provider::CalibreThemeSettings;

/// 默认书库位置。macOS 遵循惯例放在 `~/Library/Application Support/` 下。
pub fn default_library_path() -> PathBuf {
    if let Ok(path) = std::env::var("CALIBRE4RUST_LIBRARY") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home)
        .join("Library/Application Support/calibre4rust/library")
        .join("calibre.db")
}

pub fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // `Application::run` 的回调只能返回 `()`,所以开窗失败要通过共享单元带回。
    let launch_error: Rc<RefCell<Option<anyhow::Error>>> = Rc::new(RefCell::new(None));
    let launch_error_for_callback = launch_error.clone();

    gpui_platform::application()
        .with_assets(CalibreAssets)
        .run(move |cx: &mut App| {
            theme::init(LoadThemes::JustBase, cx);
            theme::set_theme_settings_provider(Box::new(CalibreThemeSettings), cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(840.0)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| RootView::new(cx)),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    *launch_error_for_callback.borrow_mut() =
                        Some(anyhow::anyhow!("打开主窗口失败: {error}"));
                }
            }
        });

    match launch_error.borrow_mut().take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct RootView {
    service: Option<Entity<LibraryService>>,
    book_count: usize,
    service_error: Option<String>,
}

impl RootView {
    fn new(cx: &mut App) -> Self {
        let library_path = default_library_path();
        match LibraryService::open_path(&library_path, 5_000) {
            Ok(service) => {
                let book_count = service.book_count().unwrap_or(0);
                Self {
                    service: Some(cx.new(|_| service)),
                    book_count,
                    service_error: None,
                }
            }
            Err(error) => {
                tracing::error!(%error, "打开书库失败");
                Self {
                    service: None,
                    book_count: 0,
                    service_error: Some(format!("{error:#}")),
                }
            }
        }
    }
}

impl Render for RootView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let book_count = self.book_count;
        let service_error = self.service_error.clone();
        let library_loaded = self.service.is_some();

        div()
            .flex_col()
            .p_4()
            .gap_2()
            .child("calibre4rust —— 中文字体测试:你好,世界")
            .child(if library_loaded {
                format!("书库中有 {book_count} 本书")
            } else {
                "书库未加载".to_string()
            })
            .when_some(service_error, |this, error| {
                this.child(div().text_color(gpui::rgba(0xE5484D)).child(error))
            })
            .child(ui::Button::new("test-button", "点击我").on_click(|_event, _window, _cx| {}))
    }
}
