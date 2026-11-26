use crate::infrastructure::db::DEFAULT_WORKSPACE_ID;
use rusqlite::{Connection, OptionalExtension, Result, params};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub workspace_id: String,
    pub browser_id: Option<String>,
    pub browser_name: Option<String>,
    pub title: String,
    pub status: String,
    pub label: String,
    pub priority: String,
    pub assignee: String,
    pub favorite: bool,
    pub progress: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateTaskRecord<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub browser_id: Option<&'a str>,
    pub title: &'a str,
    pub status: &'a str,
    pub label: &'a str,
    pub priority: &'a str,
    pub assignee: &'a str,
    pub favorite: bool,
    pub progress: i64,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTaskRecord<'a> {
    pub browser_id: Option<Option<&'a str>>,
    pub title: Option<&'a str>,
    pub status: Option<&'a str>,
    pub label: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub assignee: Option<&'a str>,
    pub favorite: Option<bool>,
    pub progress: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct BulkUpdateTasksRecord<'a> {
    pub browser_id: Option<Option<&'a str>>,
    pub status: Option<&'a str>,
    pub label: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub assignee: Option<&'a str>,
}

const TASK_SELECT_JOIN: &str = "
    SELECT t.id, t.workspace_id, t.browser_id, b.name, t.title, t.status, t.label,
           t.priority, t.assignee, t.favorite, t.progress, t.created_at, t.updated_at
    FROM tasks t
    LEFT JOIN browsers b ON b.id = t.browser_id
";

fn map_task_row(row: &rusqlite::Row<'_>) -> Result<TaskRecord> {
    Ok(TaskRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        browser_id: row.get(2)?,
        browser_name: row.get(3)?,
        title: row.get(4)?,
        status: row.get(5)?,
        label: row.get(6)?,
        priority: row.get(7)?,
        assignee: row.get(8)?,
        favorite: row.get::<_, i64>(9)? != 0,
        progress: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub fn normalize_task_progress(status: &str, progress: i64) -> i64 {
    if status == "done" {
        100
    } else if status == "canceled" {
        0
    } else {
        progress.clamp(0, 100)
    }
}

pub struct SqliteTaskRepository<'a> {
    pub conn: &'a Connection,
}

impl<'a> SqliteTaskRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn list_tasks(
        &self,
        workspace_id: Option<&str>,
        browser_id: Option<&str>,
        status: Option<&str>,
        search: Option<&str>,
    ) -> Result<Vec<TaskRecord>> {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ws) = workspace_id.map(str::trim).filter(|s| !s.is_empty()) {
            conditions.push("t.workspace_id = ?");
            param_values.push(Box::new(ws.to_string()));
        }
        if let Some(bid) = browser_id.map(str::trim).filter(|s| !s.is_empty()) {
            if bid == "__unassigned__" {
                conditions.push("t.browser_id IS NULL");
            } else {
                conditions.push("t.browser_id = ?");
                param_values.push(Box::new(bid.to_string()));
            }
        }
        if let Some(st) = status.map(str::trim).filter(|s| !s.is_empty()) {
            conditions.push("t.status = ?");
            param_values.push(Box::new(st.to_string()));
        }
        if let Some(q) = search.map(str::trim).filter(|s| !s.is_empty()) {
            conditions.push(
                "(t.id LIKE ? OR t.title LIKE ? OR t.label LIKE ? OR t.assignee LIKE ? OR COALESCE(b.name, '') LIKE ?)",
            );
            let pattern = format!("%{q}%");
            for _ in 0..5 {
                param_values.push(Box::new(pattern.clone()));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!("{TASK_SELECT_JOIN} {where_clause} ORDER BY t.rowid DESC");

        let params_slice: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_slice.as_slice(), map_task_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<TaskRecord>> {
        let sql = format!("{TASK_SELECT_JOIN} WHERE t.id = ?1");
        self.conn
            .query_row(&sql, params![id], map_task_row)
            .optional()
    }

    pub fn create_task(&self, input: &CreateTaskRecord<'_>) -> Result<TaskRecord> {
        let workspace_id = if input.workspace_id.trim().is_empty() {
            DEFAULT_WORKSPACE_ID
        } else {
            input.workspace_id.trim()
        };
        let progress = normalize_task_progress(input.status, input.progress);

        self.conn.execute(
            "INSERT INTO tasks (id, workspace_id, browser_id, title, status, label, priority, assignee, favorite, progress)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                input.id,
                workspace_id,
                input.browser_id,
                input.title,
                input.status,
                input.label,
                input.priority,
                input.assignee,
                i64::from(input.favorite),
                progress,
            ],
        )?;

        self.get_task(input.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_task(
        &self,
        id: &str,
        patch: &UpdateTaskRecord<'_>,
    ) -> Result<Option<TaskRecord>> {
        let Some(existing) = self.get_task(id)? else {
            return Ok(None);
        };

        let next_browser = match patch.browser_id {
            Some(v) => v,
            None => existing.browser_id.as_deref(),
        };
        let next_title = patch.title.unwrap_or(&existing.title);
        let next_status = patch.status.unwrap_or(&existing.status);
        let next_label = patch.label.unwrap_or(&existing.label);
        let next_priority = patch.priority.unwrap_or(&existing.priority);
        let next_assignee = patch.assignee.unwrap_or(&existing.assignee);
        let next_favorite = patch.favorite.unwrap_or(existing.favorite);
        let next_progress =
            normalize_task_progress(next_status, patch.progress.unwrap_or(existing.progress));

        self.conn.execute(
            "UPDATE tasks
             SET browser_id = ?1, title = ?2, status = ?3, label = ?4, priority = ?5,
                 assignee = ?6, favorite = ?7, progress = ?8, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?9",
            params![
                next_browser,
                next_title,
                next_status,
                next_label,
                next_priority,
                next_assignee,
                i64::from(next_favorite),
                next_progress,
                id,
            ],
        )?;

        self.get_task(id)
    }

    pub fn bulk_update(&self, ids: &[String], patch: &BulkUpdateTasksRecord<'_>) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut updated = 0;
        for id in ids {
            let task_patch = UpdateTaskRecord {
                browser_id: patch.browser_id,
                status: patch.status,
                label: patch.label,
                priority: patch.priority,
                assignee: patch.assignee,
                ..Default::default()
            };
            if self.update_task(id, &task_patch)?.is_some() {
                updated += 1;
            }
        }
        tx.commit()?;
        Ok(updated)
    }

    pub fn delete_tasks(&self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut deleted = 0;
        for id in ids {
            deleted += tx.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    pub fn unlink_browser_outside_workspace(
        &self,
        browser_id: &str,
        target_workspace_id: &str,
    ) -> Result<usize> {
        self.conn.execute(
            "UPDATE tasks
             SET browser_id = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE browser_id = ?1 AND workspace_id != ?2",
            params![browser_id, target_workspace_id],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::db::AutomationDb;

    #[test]
    fn tasks_enforce_browser_fk_join_name_and_set_null_on_delete() {
        let db = AutomationDb::new_in_memory().unwrap();
        db.browsers()
            .create_browser("prf_1", "Stealth Alpha", None, None, None, None)
            .unwrap();

        // Non-existent browser_id must fail foreign key constraint
        let bad = db.tasks().create_task(&CreateTaskRecord {
            id: "TASK-BAD",
            workspace_id: DEFAULT_WORKSPACE_ID,
            browser_id: Some("prf_missing"),
            title: "Should fail FK",
            status: "todo",
            label: "feature",
            priority: "high",
            assignee: "linh",
            favorite: false,
            progress: 0,
        });
        assert!(bad.is_err());

        // Valid browser link resolves live browser_name via LEFT JOIN
        let created = db
            .tasks()
            .create_task(&CreateTaskRecord {
                id: "TASK-1001",
                workspace_id: DEFAULT_WORKSPACE_ID,
                browser_id: Some("prf_1"),
                title: "Warmup TikTok feed",
                status: "in progress",
                label: "feature",
                priority: "high",
                assignee: "linh",
                favorite: true,
                progress: 50,
            })
            .unwrap();
        assert_eq!(created.browser_id.as_deref(), Some("prf_1"));
        assert_eq!(created.browser_name.as_deref(), Some("Stealth Alpha"));

        // Linked job also references both browser and task cleanly
        assert!(db.jobs().create_job_with_links(
            "job_linked_1",
            Some("prf_1"),
            Some("TASK-1001"),
            "Warmup TikTok feed",
            &serde_json::json!({}),
            &serde_json::json!({}),
            "running",
        ));

        // Deleting browser sets task.browser_id = NULL (preserves task)
        assert_eq!(db.browsers().delete_browser_checked("prf_1").unwrap(), 1);
        let after = db.tasks().get_task("TASK-1001").unwrap().unwrap();
        assert_eq!(after.browser_id, None);
        assert_eq!(after.browser_name, None);
    }
}
