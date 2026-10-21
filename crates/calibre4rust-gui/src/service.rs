use anyhow::Context;
use caliberate_db::database::Database;
use caliberate_library::catalog::{LibraryBook, LibraryCatalog};
use caliberate_library::summary::LibrarySummaryPage;
use rusqlite::OpenFlags;
use std::path::{Path, PathBuf};

// 纯数据类型在这里转发一次,UI 模块从 `crate::service` 取,不必直接依赖 caliberate。
// 架构边界关心的是**查询路径**(catalog / database / SQL),不是数据形状。
pub use caliberate_library::query::{
    LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibrarySortField,
};
pub use caliberate_library::summary::{LibraryBookSummary, LibrarySeriesSummary};

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
    database_path: PathBuf,
}

impl LibraryService {
    /// 打开(或创建)指定路径的书库数据库。
    ///
    /// 如果该路径是一个 **calibre 的书库**(`metadata.db`),直接拒绝 —— 打开它会触发
    /// caliberate 的 schema 迁移,那等于静默改写用户自己的 calibre 数据。
    /// calibre 库要通过只读挂载访问(caliberate 的 `CalibreLibraryBackend`),
    /// 那是后续阶段的事。
    pub fn open_path(path: &Path, busy_timeout_ms: u64) -> anyhow::Result<Self> {
        if is_foreign_library(path) {
            anyhow::bail!(
                "{} 看起来是 calibre 的书库(metadata.db)。calibre4rust 不会改写 calibre 的书库;\
                 请改用 calibre4rust 自己的书库路径。",
                path.display()
            );
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建书库目录失败: {}", parent.display()))?;
        }
        let database = Database::open_path(path, busy_timeout_ms)
            .with_context(|| format!("打开书库失败: {}", path.display()))?;
        Ok(Self {
            database,
            database_path: path.to_path_buf(),
        })
    }

    /// 打开一个临时的内存书库。用于书库不可用时的占位,数据不落盘。
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let database = Database::open_path(":memory:", 1_000).context("打开内存书库失败")?;
        Ok(Self {
            database,
            database_path: PathBuf::from(":memory:"),
        })
    }

    /// 书库数据库的路径。后台加载线程用它另开一个只读连接。
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// `LibraryCatalog` 借用 backend,不能自引用 —— 每次调用现建一个。
    fn catalog(&self) -> LibraryCatalog<'_> {
        LibraryCatalog::new(&self.database)
    }

    /// 书库里的书的总数。用 `page.total` 而不是全表 `list_books()` —— 后者会把每一本
    /// 书的完整记录都读进内存,只为了得到一个数字。
    pub fn book_total(&self) -> anyhow::Result<usize> {
        let query = default_library_query().with_limit(1);
        Ok(self.catalog().query_summary_page(&query)?.total)
    }

    /// 书库里的书的总数(兼容旧调用点)。
    pub fn book_count(&self) -> anyhow::Result<usize> {
        self.book_total()
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

/// 判断一个 SQLite 文件是不是 calibre 的书库。
///
/// calibre 的 `metadata.db` 有 `books` 表,但没有 caliberate 的 `schema_migrations` 表。
/// 判据:有 `books` 表且**没有** `schema_migrations`。
fn is_foreign_library(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(connection) = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    let has_table = |name: &str| -> bool {
        connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |_| Ok(()),
            )
            .is_ok()
    };
    has_table("books") && !has_table("schema_migrations")
}

/// 分块拉取全部书的摘要(前台)。
///
/// 不用 `list_books()` —— 大库上那是一次性全表加载。这里照搬 caliberate egui 侧
/// 的分块模式(`crates/gui/src/views.rs:84-101`)。
pub fn load_summaries(
    service: &LibraryService,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    load_summaries_with(|page_query| service.query_summary_page(page_query), query)
}

/// 在后台线程里加载书的摘要。
///
/// `rusqlite::Connection` 是 `Send`(但非 `Sync`),所以后台线程可以**另开一个连接**
/// 读同一个库,不必和 UI 线程共享连接。
pub fn load_summaries_in_background(
    database_path: &Path,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    let database = Database::open_path(database_path, 5_000)
        .with_context(|| format!("后台打开书库失败: {}", database_path.display()))?;
    let catalog = LibraryCatalog::new(&database);
    load_summaries_with(
        |page_query| Ok(catalog.query_summary_page(page_query)?),
        query,
    )
}

fn load_summaries_with(
    mut fetch: impl FnMut(&LibraryQuery) -> anyhow::Result<LibrarySummaryPage>,
    query: &LibraryQuery,
) -> anyhow::Result<Vec<LibraryBookSummary>> {
    let total = fetch(&query.clone().with_limit(1))?.total;
    let mut rows = Vec::with_capacity(total.min(SUMMARY_CHUNK_SIZE));
    for offset in chunk_offsets(total, SUMMARY_CHUNK_SIZE) {
        let page = fetch(
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

    /// 打开一个 calibre 的 `metadata.db` 绝不能改写它。
    #[test]
    fn refuses_to_open_a_calibre_library() {
        let dir = temp_dir();
        let database_path = dir.join("metadata.db");
        {
            let connection = rusqlite::Connection::open(&database_path).expect("create");
            connection
                .execute_batch(
                    "CREATE TABLE books(id INTEGER PRIMARY KEY, title TEXT, path TEXT);
                     CREATE TABLE authors(id INTEGER PRIMARY KEY, name TEXT);
                     INSERT INTO books VALUES(1, '三体', '刘慈欣/三体 (1)');",
                )
                .expect("create calibre-like schema");
        }
        let before = std::fs::read(&database_path).expect("read before");

        let message = match LibraryService::open_path(&database_path, 1_000) {
            Ok(_) => panic!("calibre 书库应被拒绝"),
            Err(error) => format!("{error:#}"),
        };
        assert!(
            message.contains("calibre"),
            "错误信息应说明这是 calibre 书库: {message}"
        );

        let after = std::fs::read(&database_path).expect("read after");
        assert_eq!(before, after, "calibre 的 metadata.db 不能被改写");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chunk_offsets_cover_the_whole_result_set() {
        let offsets = chunk_offsets(100_000, 2_000);
        assert_eq!(offsets.len(), 50);
        assert_eq!(offsets[0], 0);
        assert_eq!(*offsets.last().expect("non-empty"), 98_000);
    }
}
