use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, Modifiers, Styled};
use ui::prelude::*;

use crate::service::{LibraryBookSummary, LibraryQuery, LibraryService, LibrarySortField};

// 分块偏移量的计算放在 service 层(那里也有后台加载要用),这里转发以保持对外接口稳定。
pub use crate::service::chunk_offsets;

/// 分块拉取全部书的摘要。
pub fn load_all_summaries(
    service: &LibraryService,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    crate::service::load_summaries(service, query)
}

/// 书列表的列。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookColumn {
    Title,
    Authors,
    Series,
    Tags,
    Publisher,
    Rating,
    DateAdded,
    Format,
    Size,
}

/// 默认显示的列。
///
/// 上游 calibre 默认隐藏 `last_modified / languages / formats / id / path / pages`
/// (`calibre/src/calibre/gui2/library/views.py:1106`)。
pub fn default_visible_columns() -> Vec<BookColumn> {
    vec![
        BookColumn::Title,
        BookColumn::Authors,
        BookColumn::Series,
        BookColumn::Tags,
        BookColumn::Publisher,
        BookColumn::Rating,
        BookColumn::DateAdded,
        BookColumn::Format,
    ]
}

/// 列的稳定标识,用于持久化。
pub fn column_label(column: BookColumn) -> &'static str {
    match column {
        BookColumn::Title => "title",
        BookColumn::Authors => "authors",
        BookColumn::Series => "series",
        BookColumn::Tags => "tags",
        BookColumn::Publisher => "publisher",
        BookColumn::Rating => "rating",
        BookColumn::DateAdded => "date_added",
        BookColumn::Format => "format",
        BookColumn::Size => "size",
    }
}

/// 列对应的排序字段。`None` 表示该列不可排序。
pub fn sort_field_for(column: BookColumn) -> Option<LibrarySortField> {
    match column {
        BookColumn::Title => Some(LibrarySortField::Title),
        BookColumn::Authors => Some(LibrarySortField::Authors),
        BookColumn::Series => Some(LibrarySortField::Series),
        BookColumn::Tags => Some(LibrarySortField::Tags),
        BookColumn::Publisher => Some(LibrarySortField::Publisher),
        BookColumn::Rating => Some(LibrarySortField::Rating),
        BookColumn::DateAdded => Some(LibrarySortField::DateAdded),
        BookColumn::Format => Some(LibrarySortField::Format),
        BookColumn::Size => None,
    }
}

/// 把一行书渲染成某个单元格里的文本。
pub fn format_row_cell(column: BookColumn, summary: &LibraryBookSummary) -> String {
    match column {
        BookColumn::Title => summary.title.clone(),
        BookColumn::Authors => summary.authors.join(", "),
        BookColumn::Series => match &summary.series {
            Some(series) => format!("{} [{}]", series.name, series.index),
            None => String::new(),
        },
        BookColumn::Tags => summary.tags.join(", "),
        BookColumn::Publisher => summary.publisher.clone().unwrap_or_default(),
        // calibre 内部评分 0–10,等于 2×星。
        BookColumn::Rating => summary
            .rating
            .map(|rating| (rating / 2).to_string())
            .unwrap_or_default(),
        BookColumn::DateAdded => summary.date_added.clone().unwrap_or_default(),
        BookColumn::Format => summary
            .formats
            .iter()
            .map(|format| format.format.clone())
            .collect::<Vec<_>>()
            .join(", "),
        BookColumn::Size => String::new(),
    }
}

/// 点击时按住的修饰键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionModifiers {
    /// Cmd / Ctrl —— 反选该行。
    pub toggle: bool,
    /// Shift —— 从锚点连选。
    pub extend: bool,
}

impl From<Modifiers> for SelectionModifiers {
    fn from(modifiers: Modifiers) -> SelectionModifiers {
        SelectionModifiers {
            toggle: modifiers.secondary(),
            extend: modifiers.shift,
        }
    }
}

/// 点击某一行之后的选中集合。
///
/// - 无修饰键:只选中被点的那一行
/// - Cmd/Ctrl:切换该行的选中状态
/// - Shift:从锚点连选到被点的行
pub fn selection_after_click(
    current: &[i64],
    anchor: Option<i64>,
    clicked_id: i64,
    modifiers: SelectionModifiers,
    all_ids: &[i64],
) -> Vec<i64> {
    if modifiers.extend {
        let Some(anchor_id) = anchor else {
            return vec![clicked_id];
        };
        let Some(start) = all_ids.iter().position(|id| *id == anchor_id) else {
            return vec![clicked_id];
        };
        let Some(end) = all_ids.iter().position(|id| *id == clicked_id) else {
            return vec![clicked_id];
        };
        let (from, to) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        return all_ids[from..=to].to_vec();
    }

    if modifiers.toggle {
        let mut selection: Vec<i64> = current.to_vec();
        if let Some(position) = selection.iter().position(|id| *id == clicked_id) {
            selection.remove(position);
        } else {
            selection.push(clicked_id);
        }
        return selection;
    }

    vec![clicked_id]
}

pub struct BookList {
    service: Option<Entity<LibraryService>>,
    rows: Vec<LibraryBookSummary>,
    columns: Vec<BookColumn>,
    sort_column: BookColumn,
    sort_descending: bool,
    selected: Vec<i64>,
    selection_anchor: Option<i64>,
    last_error: Option<String>,
    loading: bool,
}

impl BookList {
    pub fn new(service: Option<Entity<LibraryService>>, cx: &mut App) -> Entity<Self> {
        cx.new(|_cx| Self {
            service,
            rows: Vec::new(),
            columns: default_visible_columns(),
            sort_column: BookColumn::Title,
            sort_descending: false,
            selected: Vec::new(),
            selection_anchor: None,
            last_error: None,
            loading: false,
        })
    }

    /// 重新加载书列表。
    ///
    /// 在后台线程另开一个连接读库,不在 UI 线程上做全表扫描。
    pub fn refresh(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.rows.clear();
            self.last_error = Some("书库未加载".to_string());
            return;
        };
        let database_path = service.read(cx).database_path().to_path_buf();
        let query = self.current_query();
        self.loading = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::service::load_summaries_in_background(&database_path, &query)
                })
                .await;
            this.update(cx, |this, _cx| {
                this.loading = false;
                match result {
                    Ok(rows) => {
                        this.rows = rows;
                        this.last_error = None;
                    }
                    Err(error) => {
                        tracing::error!(%error, "加载书列表失败");
                        this.rows.clear();
                        this.last_error = Some(format!("{error:#}"));
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// 当前排序对应的查询。
    pub fn current_query(&self) -> LibraryQuery {
        let mut query = crate::service::default_library_query();
        if let Some(sort_field) = sort_field_for(self.sort_column) {
            query.sort = sort_field;
        }
        query.descending = self.sort_descending;
        query
    }

    /// 按列排序。再次点击同一列切换升降序。
    pub fn sort_by(&mut self, column: BookColumn, cx: &mut gpui::Context<Self>) {
        if sort_field_for(column).is_none() {
            return;
        }
        if self.sort_column == column {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort_column = column;
            self.sort_descending = false;
        }
        self.refresh(cx);
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn selected_book_ids(&self) -> Vec<i64> {
        self.selected.clone()
    }

    pub fn columns(&self) -> Vec<BookColumn> {
        self.columns.clone()
    }

    /// 当前排序状态,用于持久化。返回列标识与是否降序。
    pub fn sort_state(&self) -> (String, bool) {
        (
            column_label(self.sort_column).to_string(),
            self.sort_descending,
        )
    }

    /// 从持久化状态恢复排序。
    pub fn apply_sort(
        &mut self,
        column_name: &str,
        descending: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(column) = self
            .columns
            .iter()
            .copied()
            .find(|column| column_label(*column) == column_name)
        {
            self.sort_column = column;
            self.sort_descending = descending;
        }
        self.refresh(cx);
    }

    /// 应用持久化的隐藏列。
    pub fn apply_hidden_columns(&mut self, hidden: &[String]) {
        self.columns = default_visible_columns()
            .into_iter()
            .filter(|column| !hidden.iter().any(|name| name == column_label(*column)))
            .collect();
    }

    /// 最近一次加载的错误。UI 应当把它显示给用户,而不是只写日志。
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    fn handle_row_click(
        &mut self,
        book_id: i64,
        modifiers: SelectionModifiers,
        cx: &mut gpui::Context<Self>,
    ) {
        let all_ids: Vec<i64> = self.rows.iter().map(|row| row.id).collect();
        self.selected = selection_after_click(
            &self.selected,
            self.selection_anchor,
            book_id,
            modifiers,
            &all_ids,
        );
        if !modifiers.extend {
            self.selection_anchor = Some(book_id);
        }
        cx.notify();
    }
}

impl Render for BookList {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let window = cx.entity();
        let selected = self.selected.clone();
        let columns = self.columns.clone();

        let header: Vec<AnyElement> = columns
            .iter()
            .copied()
            .map(|column| {
                let window = window.clone();
                let label = column_label(column);
                gpui::div()
                    .id(format!("book-list-header-{label}"))
                    .px_2()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(move |_event, _window, cx| {
                        window.update(cx, |this, cx| this.sort_by(column, cx));
                    })
                    .child(label)
                    .into_any_element()
            })
            .collect();

        let list = window.clone();
        let body = ui::Table::new(columns.len())
            .uniform_list(
                "book-list-table",
                self.rows.len(),
                move |range, _window, cx| {
                    // 只读需要渲染的那几行 —— 不克隆整份数据集。
                    // (悬停样式变化会触发整表重绘,全量克隆在 10 万本时是几十 MB 的分配)
                    let list = list.clone();
                    let selected = selected.clone();
                    let columns = columns.clone();
                    let book_list = list.read(cx);
                    range
                        .map(|index| {
                            let summary = &book_list.rows[index];
                            let is_selected = selected.contains(&summary.id);
                            columns
                                .iter()
                                .copied()
                                .map(|column| {
                                    let list = list.clone();
                                    let book_id = summary.id;
                                    gpui::div()
                                        .id(format!("book-cell-{index}-{}", column_label(column)))
                                        .px_2()
                                        .when(is_selected, |this| this.bg(gpui::rgba(0x3B82F633)))
                                        .on_click(move |event, _window, cx| {
                                            let modifiers =
                                                SelectionModifiers::from(event.modifiers());
                                            list.update(cx, |this, cx| {
                                                this.handle_row_click(book_id, modifiers, cx);
                                            });
                                        })
                                        .child(format_row_cell(column, summary))
                                        .into_any_element()
                                })
                                .collect()
                        })
                        .collect()
                },
            )
            .header(header);

        let last_error = self.last_error.clone();
        let loading = self.loading;

        div()
            .flex_1()
            .flex_col()
            .overflow_hidden()
            .child(body)
            .child(
                div()
                    .h(px(24.0))
                    .px_2()
                    .items_center()
                    .gap_2()
                    .when(loading, |this| this.child("加载中…"))
                    .when_some(last_error, |this, error| {
                        this.child(div().text_color(gpui::rgba(0xE5484D)).child(error))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary(title: &str) -> LibraryBookSummary {
        LibraryBookSummary {
            id: 1,
            title: title.to_string(),
            format: "EPUB".to_string(),
            path: "Author/Book (1)".to_string(),
            formats: Vec::new(),
            authors: Vec::new(),
            tags: Vec::new(),
            series: None,
            rating: None,
            publisher: None,
            languages: Vec::new(),
            has_cover: false,
            date_added: None,
            date_modified: None,
            pubdate: None,
        }
    }

    fn sample_summary_with_authors(authors: Vec<String>) -> LibraryBookSummary {
        let mut summary = sample_summary("X");
        summary.authors = authors;
        summary
    }

    #[test]
    fn default_columns_hide_upstream_hidden_fields() {
        let columns = default_visible_columns();
        let names: Vec<&str> = columns.iter().map(|c| column_label(*c)).collect();
        for hidden in [
            "last_modified",
            "languages",
            "formats",
            "id",
            "path",
            "pages",
        ] {
            assert!(!names.contains(&hidden), "不应显示 {hidden}");
        }
        assert!(names.contains(&"title"));
    }

    #[test]
    fn title_column_maps_to_title_sort_field() {
        assert_eq!(
            sort_field_for(BookColumn::Title),
            Some(LibrarySortField::Title)
        );
        assert_eq!(
            sort_field_for(BookColumn::Authors),
            Some(LibrarySortField::Authors)
        );
        assert_eq!(
            sort_field_for(BookColumn::DateAdded),
            Some(LibrarySortField::DateAdded)
        );
    }

    #[test]
    fn column_labels_are_human_readable() {
        assert_eq!(column_label(BookColumn::Title), "title");
        assert_eq!(column_label(BookColumn::DateAdded), "date_added");
    }

    #[test]
    fn row_cells_render_cjk_emoji_and_long_titles() {
        let summary = sample_summary("《三体》🚀 —— 一个非常非常长的书名用来测试超长情况");
        let cell = format_row_cell(BookColumn::Title, &summary);
        assert_eq!(cell, "《三体》🚀 —— 一个非常非常长的书名用来测试超长情况");
    }

    #[test]
    fn author_cell_joins_with_comma() {
        let summary = sample_summary_with_authors(vec!["刘慈欣".to_string(), "王晋康".to_string()]);
        assert_eq!(
            format_row_cell(BookColumn::Authors, &summary),
            "刘慈欣, 王晋康"
        );
    }

    #[test]
    fn rating_cell_divides_by_two() {
        let mut summary = sample_summary("X");
        summary.rating = Some(8);
        assert_eq!(format_row_cell(BookColumn::Rating, &summary), "4");
    }

    #[test]
    fn chunk_offsets_cover_the_whole_result_set() {
        let offsets = chunk_offsets(100_000, 2_000);
        assert_eq!(offsets.len(), 50);
        assert_eq!(offsets[0], 0);
        assert_eq!(*offsets.last().expect("non-empty"), 98_000);
        for pair in offsets.windows(2) {
            assert_eq!(pair[1] - pair[0], 2_000);
        }
    }

    #[test]
    fn chunk_offsets_handle_small_and_empty_result_sets() {
        assert_eq!(chunk_offsets(0, 2_000), Vec::<usize>::new());
        assert_eq!(chunk_offsets(10, 2_000), vec![0]);
        assert_eq!(chunk_offsets(2_000, 2_000), vec![0]);
        assert_eq!(chunk_offsets(2_001, 2_000), vec![0, 2_000]);
    }

    #[test]
    fn plain_click_selects_only_the_clicked_row() {
        let selection = selection_after_click(
            &[1, 2],
            Some(1),
            3,
            SelectionModifiers::default(),
            &[1, 2, 3],
        );
        assert_eq!(selection, vec![3]);
    }

    #[test]
    fn toggle_click_adds_and_removes_the_row() {
        let modifiers = SelectionModifiers {
            toggle: true,
            extend: false,
        };
        let selection = selection_after_click(&[1], Some(1), 3, modifiers, &[1, 2, 3]);
        assert_eq!(selection, vec![1, 3]);
        let selection = selection_after_click(&selection, Some(1), 3, modifiers, &[1, 2, 3]);
        assert_eq!(selection, vec![1]);
    }

    #[test]
    fn extend_click_selects_the_range_from_the_anchor() {
        let modifiers = SelectionModifiers {
            toggle: false,
            extend: true,
        };
        let selection = selection_after_click(&[], Some(1), 4, modifiers, &[1, 2, 3, 4, 5]);
        assert_eq!(selection, vec![1, 2, 3, 4]);
    }

    #[test]
    fn extend_click_backwards_selects_the_range_too() {
        let modifiers = SelectionModifiers {
            toggle: false,
            extend: true,
        };
        let selection = selection_after_click(&[], Some(4), 2, modifiers, &[1, 2, 3, 4, 5]);
        assert_eq!(selection, vec![2, 3, 4]);
    }
}
