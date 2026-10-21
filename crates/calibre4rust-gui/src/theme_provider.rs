use gpui::{App, Font, FontFallbacks, Pixels, px};
use std::sync::OnceLock;
use theme::{ThemeSettingsProvider, UiDensity};

pub struct CalibreThemeSettings;

impl ThemeSettingsProvider for CalibreThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        static UI_FONT: OnceLock<Font> = OnceLock::new();
        UI_FONT.get_or_init(|| Font {
            family: ".SystemUIFont".into(),
            features: Default::default(),
            fallbacks: Some(FontFallbacks::from_fonts(vec![
                "PingFang SC".to_string(),
                "Hiragino Sans GB".to_string(),
                "STHeiti".to_string(),
            ])),
            weight: Default::default(),
            style: Default::default(),
        })
    }

    fn buffer_font<'a>(&'a self, cx: &'a App) -> &'a Font {
        self.ui_font(cx)
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(13.0)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::default()
    }
}
