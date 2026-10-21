use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, Styled};
use ui::prelude::*;

use crate::service::SUMMARY_CHUNK_SIZE;
use crate::service::{LibraryBookSummary, LibraryQuery, LibraryService, LibrarySortField};

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

/// 把总数按块大小切成连续的偏移量。
pub fn chunk_offsets(total: usize, chunk_size: usize) -> Vec<usize> {
    if chunk_size == 0 || total == 0 {
        return Vec::new();
    }
    let mut offsets = Vec::new();
    let mut offset = 0usize;
    while offset < total {
        offsets.push(offset);
        offset += chunk_size;
    }
    offsets
}

/// 分块拉取全部书的摘要。
///
/// 不用 `list_books()` —— 大库上那是一次性全表加载。这里照搬 caliberate egui 侧
/// 的分块模式(`crates/gui/src/views.rs:84-101`)。
pub fn load_all_summaries(
    service: &LibraryService,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    let first_page = service.query_summary_page(&query.clone().with_limit(1))?;
    let total = first_page.total;
    let mut rows = Vec::with_capacity(total.min(SUMMARY_CHUNK_SIZE));
    for offset in chunk_offsets(total, SUMMARY_CHUNK_SIZE) {
        let page = service.query_summary_page(
            &query
                .clone()
                .with_limit(SUMMARY_CHUNK_SIZE)
                .with_offset(offset),
        )?;
        let fetched = page.books.len();
        rows.extend(page.books);
        if fetched < SUMMARY_CHUNK_SIZE {
            break;
        }
    }
    Ok(rows)
}

pub struct BookList {
    service: Option<Entity<LibraryService>>,
    rows: Vec<LibraryBookSummary>,
    columns: Vec<BookColumn>,
    sort_column: BookColumn,
    sort_descending: bool,
    selected: Vec<i64>,
}

impl BookList {
    pub fn new(service: Option<Entity<LibraryService>>, cx: &mut App) -> Entity<Self> {
        let list = cx.new(|_cx| Self {
            service,
            rows: Vec::new(),
            columns: default_visible_columns(),
            sort_column: BookColumn::Title,
            sort_descending: false,
            selected: Vec::new(),
        });
        list.update(cx, |this, cx| this.refresh(cx));
        list
    }

    /// 重新加载书列表。
    pub fn refresh(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.rows.clear();
            return;
        };
        let query = self.current_query();
        let service = service.read(cx);
        match load_all_summaries(service, &query) {
            Ok(rows) => self.rows = rows,
            Err(error) => {
                tracing::error!(%error, "加载书列表失败");
                self.rows.clear();
            }
        }
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
}

impl Render for BookList {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let window = cx.entity();
        let selected_count = self.selected.len();

        let header_columns = self.columns.clone();
        let header: Vec<AnyElement> = header_columns
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

        let body_rows = self.rows.clone();
        let row_total = body_rows.len();
        let body_columns = self.columns.clone();
        let body = ui::Table::new(body_columns.len())
            .uniform_list(
                "book-list-table",
                body_rows.len(),
                move |range, _window, _cx| {
                    range
                        .map(|index| {
                            let summary = &body_rows[index];
                            body_columns
                                .iter()
                                .copied()
                                .map(|column| {
                                    gpui::div()
                                        .px_2()
                                        .child(format_row_cell(column, summary))
                                        .into_any_element()
                                })
                                .collect()
                        })
                        .collect()
                },
            )
            .header(header);

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
                    .child(format!("{row_total} 本书,已选 {selected_count}")),
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
}
