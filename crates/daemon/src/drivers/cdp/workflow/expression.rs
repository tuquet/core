use super::ExecutionState;
use regex::Regex;
use serde_json::{Map, Value};

/// Recursively interpolates `{{variables.x}}`, `{{table.0.col}}`, `{{loopData.id}}`, etc.
pub(super) fn interpolate_string(input: &str, state: &ExecutionState) -> String {
    let re = Regex::new(r"\{\{([^{}]+)\}\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let expr = caps[1].trim();
        resolve_expression(expr, state)
    })
    .to_string()
}

pub(super) fn resolve_expression(expr: &str, state: &ExecutionState) -> String {
    if expr == "$timestamp" {
        return chrono::Utc::now().timestamp_millis().to_string();
    }
    if expr == "$date" {
        return chrono::Utc::now().format("%Y-%m-%d").to_string();
    }

    if let Some(var_expr) = expr
        .strip_prefix("variables.")
        .or_else(|| expr.strip_prefix("vars."))
    {
        if let Some(v) = state.variables.get(var_expr) {
            return match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }
        return String::new();
    }

    if let Some(key) = expr.strip_prefix("googleSheets.") {
        // Automa: `referenceData.googleSheets[refKey]` (set by the `google-sheets` block).
        return match state.google_sheets_refs.get(key.trim()) {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
    }

    if let Some(rest) = expr.strip_prefix("workflow.") {
        // Automa stores a finished `execute-workflow` run as
        // `referenceData.workflow[key] = { table, variables }`.
        let mut parts = rest.splitn(2, '.');
        let key = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");
        let Some(entry) = state.workflows.get(key) else {
            return String::new();
        };
        if path.is_empty() {
            return entry.to_string();
        }
        let resolved = path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .try_fold(entry, |value, segment| match segment.parse::<usize>() {
                Ok(index) => value.get(index).or_else(|| value.get(segment)),
                Err(_) => value.get(segment),
            });
        return match resolved {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
    }

    if let Some(loop_expr) = expr.strip_prefix("loopData.") {
        let parts: Vec<&str> = loop_expr.split('.').collect();
        if let Some(loop_id) = parts.first() {
            if let Some(ld) = state.loop_data.get(*loop_id) {
                if parts.len() == 1 {
                    return ld.to_string();
                }
                if parts.len() >= 2 {
                    let sub = parts[1];
                    if sub == "$index" {
                        if let Some(idx) = ld.get("$index") {
                            return idx.to_string();
                        }
                    } else if sub == "data" {
                        if let Some(d) = ld.get("data") {
                            if parts.len() == 2 {
                                return match d {
                                    Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                };
                            }
                            if parts.len() > 2 {
                                let field = parts[2];
                                if let Some(fv) = d.get(field) {
                                    return match fv {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }
        return String::new();
    }

    if expr == "prevBlockData" {
        return match &state.prev_block_data {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
    }

    if let Some(stripped) = expr.strip_prefix("prevBlockData.") {
        if let Some(fv) = state.prev_block_data.get(stripped) {
            return match fv {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }
    }

    if let Some(gd_expr) = expr.strip_prefix("globalData.") {
        if let Some(v) = state.variables.get(gd_expr) {
            return match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }
    }

    // Automa dialect: `{{secrets@name}}`. The graph only ever holds the reference;
    // the value is resolved here from the Secret Store and scrubbed by the masker.
    if let Some(sec_expr) = expr.strip_prefix("secrets@") {
        if let Some(v) = state.secrets.get(sec_expr) {
            return match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }
        return String::new();
    }

    String::new()
}

pub(super) fn interpolate_value(val: &Value, state: &ExecutionState) -> Value {
    match val {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with("{{")
                && trimmed.ends_with("}}")
                && !trimmed[2..trimmed.len() - 2].contains("{{")
            {
                let expr = trimmed[2..trimmed.len() - 2].trim();
                if let Some(var_expr) = expr
                    .strip_prefix("variables.")
                    .or_else(|| expr.strip_prefix("vars."))
                {
                    if let Some(v) = state.variables.get(var_expr) {
                        return v.clone();
                    }
                }
                if expr == "prevBlockData" {
                    return state.prev_block_data.clone();
                }
            }
            Value::String(interpolate_string(s, state))
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|item| interpolate_value(item, state))
                .collect(),
        ),
        Value::Object(obj) => {
            let mut out = Map::new();
            for (k, v) in obj {
                out.insert(k.clone(), interpolate_value(v, state));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}
