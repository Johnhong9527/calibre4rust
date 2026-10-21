use anyhow::Context;
use caliberate_db::database::Database;
use caliberate_library::catalog::{LibraryBook, LibraryCatalog};
use caliberate_library::query::{
    LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibrarySortField,
};
use caliberate_library::summary::LibrarySummaryPage;
use std::path::Path;

pub const SUMMARY_CHUNK_SIZE: usize = 2000;

/// 书库默认查询:按书名升序,不施加任何过滤。
pub fn default_library_query() -> LibraryQuery {
    LibraryQuery {
        sort: LibrarySortField::Title,
        descending: false,
        ..Default::default()
    }
}

/// calibre 后端的薄封装。
///
/// 只在这里接触 `caliberate_*` crate —— 架构边界由 `lib.rs` 的 `architecture_tests` 强制。
pub struct LibraryService {
    database: Database,
}

impl LibraryService {
    /// 打开(或创建)指定路径的书库数据库。
    pub fn open_path(path: &Path, busy_timeout_ms: u64) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建书库目录失败: {}", parent.display()))?;
        }
        let database = Database::open_path(path, busy_timeout_ms)
            .with_context(|| format!("打开书库失败: {}", path.display()))?;
        Ok(Self { database })
    }

    /// `LibraryCatalog` 借用 backend,不能自引用 —— 每次调用现建一个。
    fn catalog(&self) -> LibraryCatalog<'_> {
        LibraryCatalog::new(&self.database)
    }

    pub fn book_count(&self) -> anyhow::Result<usize> {
        Ok(self.catalog().list_books()?.len())
    }

    pub fn query_summary_page(&self, query: &LibraryQuery) -> anyhow::Result<LibrarySummaryPage> {
        Ok(self.catalog().query_summary_page(query)?)
    }

    pub fn list_facets(&self, kind: LibraryFacetKind) -> anyhow::Result<Vec<LibraryFacetValue>> {
        Ok(self.catalog().list_facets(kind)?)
    }

    pub fn get_book(&self, id: i64) -> anyhow::Result<Option<LibraryBook>> {
        Ok(self.catalog().get_book(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("calibre4rust-svc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn default_query_sorts_by_title_ascending() {
        let query = default_library_query();
        assert_eq!(query.sort, LibrarySortField::Title);
        assert!(!query.descending);
    }

    #[test]
    fn default_query_has_no_filters() {
        let query = default_library_query();
        assert!(query.title.is_none());
        assert!(query.author.is_none());
        assert!(query.metadata_filters.is_empty());
        assert!(query.limit.is_none());
    }

    #[test]
    fn opening_a_missing_library_directory_creates_it() {
        let dir = std::env::temp_dir().join(format!("calibre4rust-svc-{}", uuid::Uuid::new_v4()));
        let database_path = dir.join("nested").join("calibre.db");
        let service = LibraryService::open_path(&database_path, 1_000).expect("open should create");
        assert_eq!(service.book_count().expect("count"), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn opening_a_corrupt_database_is_an_error_not_a_panic() {
        let dir = temp_dir();
        let database_path = dir.join("calibre.db");
        std::fs::write(&database_path, b"this is not a sqlite database").expect("write");
        let result = std::panic::catch_unwind(|| LibraryService::open_path(&database_path, 1_000));
        assert!(result.is_ok(), "打开损坏库不应 panic");
        assert!(result.expect("no panic").is_err(), "打开损坏库应返回 Err");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failing_database_open_yields_a_readable_message() {
        let dir = temp_dir();
        let database_path = dir.join("calibre.db");
        std::fs::write(&database_path, b"not a database").expect("write");
        let message = match LibraryService::open_path(&database_path, 1_000) {
            Ok(_) => panic!("应当失败"),
            Err(error) => format!("{error:#}"),
        };
        assert!(message.contains("书库"), "错误信息应提到书库: {message}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
