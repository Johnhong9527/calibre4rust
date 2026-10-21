use gpui::{App, Bounds, WindowBounds, WindowOptions, size};
use std::cell::RefCell;
use std::rc::Rc;
use theme::LoadThemes;
use ui::prelude::*;

use crate::assets::CalibreAssets;
use crate::theme_provider::CalibreThemeSettings;

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
                |_, cx| cx.new(|_| RootView),
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

struct RootView;

impl Render for RootView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_col()
            .p_4()
            .gap_2()
            .child("calibre4rust —— 中文字体测试:你好,世界")
            .child(ui::Button::new("test-button", "点击我").on_click(|_event, _window, _cx| {}))
    }
}
