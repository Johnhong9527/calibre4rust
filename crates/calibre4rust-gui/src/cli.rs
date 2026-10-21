//! 命令行入口。
//!
//! calibre 把 CLI 当作一等公民(`calibre/src/calibre/db/cli/`),caliberate 同样有
//! `calibredb`。本模块提供最基本的一条:列出书库里的书。

use crate::book_list::load_all_summaries;
use crate::service::{LibraryService, default_library_query};

/// 处理命令行参数。返回 `true` 表示已处理,不应再启动 GUI。
pub fn run(arguments: &[String]) -> anyhow::Result<bool> {
    let Some(subcommand) = arguments.get(1) else {
        return Ok(false);
    };
    match subcommand.as_str() {
        "list" => {
            list_books()?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn list_books() -> anyhow::Result<()> {
    let library_path = crate::app::default_library_path();
    let service = LibraryService::open_path(&library_path, 5_000)?;
    let query = default_library_query();
    let rows = load_all_summaries(&service, &query)?;
    for row in &rows {
        println!(
            "{}\t{}\t{}\t{}",
            row.id,
            row.title,
            row.format,
            row.authors.join(", ")
        );
    }
    eprintln!("共 {} 本", rows.len());
    Ok(())
}
