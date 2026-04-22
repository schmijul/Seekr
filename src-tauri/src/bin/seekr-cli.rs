use seekr_lib::core::{ingest, query};
use seekr_lib::db;
use std::env;
use std::path::PathBuf;

fn usage() -> String {
    [
        "seekr-cli commands:",
        "  snapshot --root <path> [--db <path>] [--index-dir <path>]",
        "  search-text --root <path> --query <text> [--limit <n>] [--db <path>] [--index-dir <path>]",
        "  search-symbols --root <path> --query <text> [--limit <n>] [--db <path>] [--index-dir <path>]",
    ]
    .join("\n")
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find_map(|w| if w[0] == key { Some(w[1].clone()) } else { None })
}

fn parse_limit(args: &[String]) -> usize {
    arg_value(args, "--limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50)
}

fn resolve_paths(args: &[String], root: &str) -> (PathBuf, PathBuf) {
    let default_base = PathBuf::from(root).join(".seekr");
    let db_path = arg_value(args, "--db")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_base.join("seekr.db"));
    let index_dir = arg_value(args, "--index-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_base.join("seekr-index"));
    (db_path, index_dir)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(usage());
    }
    let cmd = args[1].as_str();
    let root = arg_value(&args, "--root").ok_or_else(usage)?;
    let (db_path, index_dir) = resolve_paths(&args, &root);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create db dir failed: {e}"))?;
    }
    std::fs::create_dir_all(&index_dir).map_err(|e| format!("create index dir failed: {e}"))?;
    let conn = db::open_or_create(&db_path)?;

    match cmd {
        "snapshot" => {
            let report = ingest::build_snapshot(&conn, &index_dir, &root)?;
            println!(
                "snapshot={} workspace={} seen={} indexed={} failed={} removed={}",
                report.snapshot_id,
                report.workspace_id,
                report.files_seen,
                report.files_indexed,
                report.files_failed,
                report.removed
            );
            Ok(())
        }
        "search-text" => {
            let query_text = arg_value(&args, "--query").ok_or_else(usage)?;
            let workspace_id = ingest::resolve_workspace_id(&conn, &root)?;
            let hits = query::search_text(&index_dir, &workspace_id, &query_text, parse_limit(&args))?;
            for hit in hits {
                println!(
                    "[{}] {}:{}-{} score={:.3} id={} {}",
                    hit.kind, hit.path, hit.start_line, hit.end_line, hit.score, hit.entity_id, hit.symbol_name
                );
            }
            Ok(())
        }
        "search-symbols" => {
            let query_text = arg_value(&args, "--query").ok_or_else(usage)?;
            let workspace_id = ingest::resolve_workspace_id(&conn, &root)?;
            let hits = query::search_symbols(&index_dir, &workspace_id, &query_text, parse_limit(&args))?;
            for hit in hits {
                println!(
                    "[{}] {}:{}-{} score={:.3} id={} {}",
                    hit.kind, hit.path, hit.start_line, hit.end_line, hit.score, hit.entity_id, hit.symbol_name
                );
            }
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
