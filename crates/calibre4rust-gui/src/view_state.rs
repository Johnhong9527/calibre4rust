use anyhow::Context;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 书列表的最小可用宽度。面板再宽也不能把它挤没。
pub const MIN_BOOK_LIST_WIDTH: f32 = 320.0;
/// 列宽下限。
pub const MIN_COLUMN_WIDTH: f32 = 24.0;
/// 列宽上限。
pub const MAX_COLUMN_WIDTH: f32 = 1200.0;

fn default_left_panel_width() -> f32 {
    280.0
}

fn default_right_panel_width() -> f32 {
    360.0
}

fn default_layout() -> String {
    "wide".to_string()
}

fn default_view_mode() -> String {
    "table".to_string()
}

fn default_sort_column() -> String {
    "title".to_string()
}

/// 面板尺寸、列宽、可见性、视图模式的持久化状态。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewState {
    #[serde(default = "default_left_panel_width")]
    pub left_panel_width: f32,
    #[serde(default = "default_right_panel_width")]
    pub right_panel_width: f32,
    #[serde(default = "default_true")]
    pub tag_browser_visible: bool,
    #[serde(default = "default_true")]
    pub book_details_visible: bool,
    #[serde(default)]
    pub cover_browser_visible: bool,
    #[serde(default)]
    pub quick_view_visible: bool,
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default = "default_view_mode")]
    pub view_mode: String,
    #[serde(default)]
    pub column_widths: BTreeMap<String, f32>,
    #[serde(default)]
    pub hidden_columns: Vec<String>,
    #[serde(default = "default_sort_column")]
    pub selected_sort_column: String,
    #[serde(default)]
    pub sort_descending: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ViewState {
    fn default() -> ViewState {
        ViewState {
            left_panel_width: default_left_panel_width(),
            right_panel_width: default_right_panel_width(),
            tag_browser_visible: true,
            book_details_visible: true,
            cover_browser_visible: false,
            quick_view_visible: false,
            layout: default_layout(),
            view_mode: default_view_mode(),
            column_widths: BTreeMap::new(),
            hidden_columns: Vec::new(),
            selected_sort_column: default_sort_column(),
            sort_descending: false,
        }
    }
}

impl ViewState {
    /// 状态文件位置:与书库数据库同目录。
    pub fn state_path(library_path: &Path) -> PathBuf {
        library_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("view-state.json")
    }

    /// 读取状态。文件不存在或内容损坏时返回默认值 —— 状态文件坏了不该让应用起不来。
    pub fn load(path: &Path) -> ViewState {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return ViewState::default();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建状态目录失败: {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
            .with_context(|| format!("写入状态文件失败: {}", path.display()))?;
        Ok(())
    }

    /// 按当前窗口尺寸收敛持久化的几何。
    ///
    /// 窗口变小后,旧的面板尺寸不能把书列表挤没 —— 这也是 caliberate 的要求
    /// (*"Their saved geometry must be clamped against the current window"*,
    /// `docs/project/philosophy.md`)。
    pub fn clamp_to_window(&mut self, window_width: f32, _window_height: f32) {
        let window_width = if window_width.is_finite() {
            window_width
        } else {
            0.0
        };

        let available = (window_width - MIN_BOOK_LIST_WIDTH).max(0.0);
        self.left_panel_width = clamp_finite(self.left_panel_width, 0.0, available);
        self.right_panel_width = clamp_finite(self.right_panel_width, 0.0, available);

        let total = self.left_panel_width + self.right_panel_width;
        if total > available && total > 0.0 {
            let scale = available / total;
            self.left_panel_width *= scale;
            self.right_panel_width = available - self.left_panel_width;
        }

        for width in self.column_widths.values_mut() {
            *width = clamp_finite(*width, MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }
    }

    /// 布局是"wide"还是"narrow"。
    pub fn is_narrow_layout(&self) -> bool {
        self.layout == "narrow"
    }

    /// 视图模式是"grid"还是"table"。
    pub fn is_grid_view(&self) -> bool {
        self.view_mode == "grid"
    }
}

fn clamp_finite(value: f32, minimum: f32, maximum: f32) -> f32 {
    if !value.is_finite() {
        return minimum;
    }
    value.clamp(minimum, maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("calibre4rust-vs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn missing_file_yields_default_state() {
        let state = ViewState::load(Path::new("/nonexistent/calibre4rust-view-state.json"));
        assert_eq!(
            state.left_panel_width,
            ViewState::default().left_panel_width
        );
    }

    #[test]
    fn malformed_file_yields_default_state() {
        let dir = temp_dir();
        let path = dir.join("state.json");
        std::fs::write(&path, "{ not json").expect("write");
        let state = ViewState::load(&path);
        assert_eq!(state, ViewState::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trips_through_json() {
        let dir = temp_dir();
        let path = dir.join("state.json");
        let state = ViewState {
            left_panel_width: 321.0,
            hidden_columns: vec!["publisher".to_string()],
            ..ViewState::default()
        };
        state.save(&path).expect("save");
        let loaded = ViewState::load(&path);
        assert_eq!(loaded.left_panel_width, 321.0);
        assert_eq!(loaded.hidden_columns, vec!["publisher".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clamping_keeps_the_book_list_usable() {
        let mut state = ViewState {
            left_panel_width: 2000.0,
            right_panel_width: 2000.0,
            ..ViewState::default()
        };
        state.clamp_to_window(1000.0, 800.0);
        assert!(state.left_panel_width + state.right_panel_width <= 1000.0 - MIN_BOOK_LIST_WIDTH);
        assert!(state.left_panel_width >= 0.0);
    }

    #[test]
    fn clamping_normalises_absurd_column_widths() {
        let mut column_widths = BTreeMap::new();
        column_widths.insert("title".to_string(), -50.0);
        column_widths.insert("authors".to_string(), 99999.0);
        let mut state = ViewState {
            column_widths,
            ..ViewState::default()
        };
        state.clamp_to_window(1000.0, 800.0);
        assert_eq!(state.column_widths.get("title"), Some(&MIN_COLUMN_WIDTH));
        assert_eq!(state.column_widths.get("authors"), Some(&MAX_COLUMN_WIDTH));
    }

    #[test]
    fn clamping_is_idempotent() {
        let mut state = ViewState {
            left_panel_width: 5000.0,
            ..ViewState::default()
        };
        state.clamp_to_window(1000.0, 800.0);
        let once = state.clone();
        state.clamp_to_window(1000.0, 800.0);
        assert_eq!(once, state);
    }
}
