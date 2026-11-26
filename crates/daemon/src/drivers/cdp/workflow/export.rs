use serde_json::Value;

/// Renders column-oriented Automa table data / row objects as CSV.
///
/// Header order follows the workflow's declared `table` columns first (Automa
/// `workflow.table`), then any extra keys encountered, so a declared-but-empty table
/// still produces a header row.
pub(super) fn rows_to_csv(
    rows: &[Value],
    declared_columns: &[String],
    delimiter: &str,
    add_bom: bool,
) -> String {
    let mut columns: Vec<String> = declared_columns.to_vec();
    for row in rows {
        if let Some(obj) = row.as_object() {
            for key in obj.keys() {
                if !columns.iter().any(|c| c == key) {
                    columns.push(key.clone());
                }
            }
        }
    }

    let mut out = String::new();
    if add_bom {
        out.push('\u{feff}');
    }
    if !columns.is_empty() {
        out.push_str(
            &columns
                .iter()
                .map(|c| csv_cell(c, delimiter))
                .collect::<Vec<_>>()
                .join(delimiter),
        );
        out.push('\n');
    }
    for row in rows {
        let cells = columns
            .iter()
            .map(|col| csv_cell(&row.get(col).map(cell_text).unwrap_or_default(), delimiter))
            .collect::<Vec<_>>();
        out.push_str(&cells.join(delimiter));
        out.push('\n');
    }
    out
}

/// RFC-4180 quoting: only quote when the cell contains the delimiter, a quote or a newline.
fn csv_cell(value: &str, delimiter: &str) -> String {
    if value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Automa `plain-text` exports: all values flattened and joined with a single space.
pub(super) fn rows_to_plain_text(rows: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for row in rows {
        match row {
            Value::Object(obj) => {
                for value in obj.values() {
                    parts.push(cell_text(value));
                }
            }
            other => parts.push(cell_text(other)),
        }
    }
    parts.join(" ")
}

/// Writes an export artefact to disk.
///
/// Automa hands the bytes to the browser's download API; a headless daemon has no such
/// surface, so the file is written under `EXPORT_DIR` (default `~/.operon/automation/exports`)
/// and the path is returned. `onConflict` mirrors Automa: `uniquify` (default) keeps
/// existing files and appends `-1`, `-2`…; `overwrite` replaces.
pub(super) fn write_export_file(
    base_name: &str,
    ext: &str,
    content: &str,
    on_conflict: &str,
) -> Result<String, String> {
    let dir = std::env::var("EXPORT_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home).join(".operon/automation/exports")
        });
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("export-data: cannot create {}: {e}", dir.display()))?;

    // Never let a canvas-authored name escape the export directory.
    let safe: String = base_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("unnamed")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = if safe.is_empty() {
        "unnamed".to_string()
    } else {
        safe
    };

    let mut path = dir.join(format!("{safe}.{ext}"));
    if on_conflict != "overwrite" {
        let mut counter = 1;
        while path.exists() {
            path = dir.join(format!("{safe}-{counter}.{ext}"));
            counter += 1;
        }
    }

    std::fs::write(&path, content)
        .map_err(|e| format!("export-data: cannot write {}: {e}", path.display()))?;
    Ok(path.to_string_lossy().to_string())
}
