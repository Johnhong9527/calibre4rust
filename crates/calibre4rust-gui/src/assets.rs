use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

include!(concat!(env!("OUT_DIR"), "/icons_generated.rs"));

pub struct CalibreAssets;

/// 按资源路径取图标字节。路径形如 `icons/book.svg`。
pub fn icon_bytes(name: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .find(|(path, _)| *path == name)
        .map(|(_, bytes)| *bytes)
}

impl AssetSource for CalibreAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let prefix = format!("{path}/");
        Ok(ICONS
            .iter()
            .filter_map(|(icon_path, _)| icon_path.strip_prefix(&prefix))
            .map(SharedString::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_known_calibre_icon() {
        let assets = CalibreAssets;
        let bytes = assets.load("icons/book.svg").expect("load should succeed");
        assert!(bytes.is_some(), "icons/book.svg 必须存在");
        assert!(
            bytes.expect("icon").starts_with(b"<svg"),
            "图标应为 SVG 内容"
        );
    }

    #[test]
    fn list_returns_entries_under_a_directory() {
        let assets = CalibreAssets;
        let names = assets.list("icons").expect("list should succeed");
        assert!(names.contains(&"book.svg".into()), "list 应包含 book.svg");
    }

    #[test]
    fn load_returns_none_for_missing_asset() {
        let assets = CalibreAssets;
        assert!(
            assets
                .load("icons/does_not_exist.svg")
                .expect("load should succeed")
                .is_none()
        );
    }

    #[test]
    fn calibre_theme_variants_are_present() {
        assert!(icon_bytes("icons/close-for-dark-theme.svg").is_some());
    }
}
