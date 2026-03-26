use rusqlite::{params, Connection};

pub fn ensure_config_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS indexed_roots (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE
        );
        "#,
    )
    .map_err(|e| format!("config schema init failed: {e}"))
}

pub fn set_roots(conn: &mut Connection, roots: &[String]) -> Result<(), String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("open transaction failed: {e}"))?;

    tx.execute("DELETE FROM indexed_roots", [])
        .map_err(|e| format!("clear indexed roots failed: {e}"))?;

    {
        let mut stmt = tx
            .prepare("INSERT OR IGNORE INTO indexed_roots(path) VALUES (?1)")
            .map_err(|e| format!("prepare root insert failed: {e}"))?;

        for root in roots {
            stmt.execute(params![root])
                .map_err(|e| format!("insert indexed root failed: {e}"))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("commit indexed roots failed: {e}"))
}

pub fn get_roots(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM indexed_roots ORDER BY path")
        .map_err(|e| format!("prepare root select failed: {e}"))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query roots failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("read root row failed: {e}"))?);
    }

    Ok(out)
}
