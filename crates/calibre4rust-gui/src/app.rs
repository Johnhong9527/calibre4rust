use gpui::{App, Bounds, Entity, KeyBinding, WindowBounds, WindowOptions, size};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use theme::LoadThemes;
use ui::prelude::*;

use crate::actions;
use crate::assets::CalibreAssets;
use crate::service::LibraryService;
use crate::theme_provider::CalibreThemeSettings;
use crate::window::MainWindow;

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
            cx.set_menus(MainWindow::native_menus());
            cx.bind_keys([
                KeyBinding::new("alt-shift-l", actions::ToggleLayout, None),
                KeyBinding::new("alt-shift-g", actions::ToggleCoverGrid, None),
                KeyBinding::new("cmd-,", actions::OpenPreferences, None),
            ]);

            let bounds = Bounds::centered(None, size(px(1280.0), px(840.0)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let (service, error) = open_library(cx);
                    MainWindow::new(service, error, cx)
                },
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

/// 打开书库。失败时退回一个空的内存书库,并把错误信息交给窗口展示。
fn open_library(cx: &mut App) -> (Option<Entity<LibraryService>>, Option<String>) {
    let library_path = default_library_path();
    match LibraryService::open_path(&library_path, 5_000) {
        Ok(service) => (Some(cx.new(|_| service)), None),
        Err(error) => {
            tracing::error!(%error, "打开书库失败");
            let message = format!("{error:#}");
            let service = match LibraryService::open_in_memory() {
                Ok(service) => Some(cx.new(|_| service)),
                Err(fallback) => {
                    tracing::error!(%fallback, "打开内存书库也失败");
                    None
                }
            };
            (service, Some(message))
        }
    }
}
