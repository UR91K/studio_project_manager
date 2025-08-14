use crate::database::models::SqlDateTime;
use crate::error::DatabaseError;
use chrono::Local;
use log::debug;
use rusqlite::params;
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    pub fn add_task(
        &mut self,
        project_id: &str,
        description: &str,
    ) -> Result<String, DatabaseError> {
        debug!("Adding task to project {}: {}", project_id, description);
        let task_id = Uuid::new_v4().to_string();
        let now = Local::now();

        self.conn.execute(
            "INSERT INTO project_tasks (id, project_id, description, completed, created_at) VALUES (?, ?, ?, ?, ?)",
            params![task_id, project_id, description, false, SqlDateTime::from(now)],
        )?;

        debug!("Successfully added task: {}", task_id);
        Ok(task_id)
    }

    pub fn complete_task(&mut self, task_id: &str, completed: bool) -> Result<(), DatabaseError> {
        debug!(
            "Setting task {} completion status to {}",
            task_id, completed
        );
        self.conn.execute(
            "UPDATE project_tasks SET completed = ? WHERE id = ?",
            params![completed, task_id],
        )?;
        debug!("Successfully updated task completion status");
        Ok(())
    }

    pub fn update_task_description(
        &mut self,
        task_id: &str,
        description: &str,
    ) -> Result<(), DatabaseError> {
        debug!("Updating task {} description to: {}", task_id, description);
        self.conn.execute(
            "UPDATE project_tasks SET description = ? WHERE id = ?",
            params![description, task_id],
        )?;
        debug!("Successfully updated task description");
        Ok(())
    }

    pub fn get_task(
        &mut self,
        task_id: &str,
    ) -> Result<Option<(String, String, String, bool, i64)>, DatabaseError> {
        debug!("Getting task by ID: {}", task_id);
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, description, completed, created_at FROM project_tasks WHERE id = ?"
        )?;

        let result = stmt.query_row([task_id], |row| {
            let id: String = row.get(0)?;
            let project_id: String = row.get(1)?;
            let description: String = row.get(2)?;
            let completed: bool = row.get(3)?;
            let created_at: i64 = row.get(4)?;
            debug!(
                "Found task: {} ({}) for project {}",
                description, id, project_id
            );
            Ok((id, project_id, description, completed, created_at))
        });

        match result {
            Ok(task) => {
                debug!("Successfully retrieved task");
                Ok(Some(task))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                debug!("Task not found");
                Ok(None)
            }
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    pub fn remove_task(&mut self, task_id: &str) -> Result<(), DatabaseError> {
        debug!("Removing task {}", task_id);
        self.conn
            .execute("DELETE FROM project_tasks WHERE id = ?", [task_id])?;
        debug!("Successfully removed task");
        Ok(())
    }

    pub fn get_project_tasks(
        &mut self,
        project_id: &str,
    ) -> Result<Vec<(String, String, bool, i64)>, DatabaseError> {
        debug!("Getting tasks for project {}", project_id);
        let mut stmt = self.conn.prepare(
            "SELECT id, description, completed, created_at FROM project_tasks WHERE project_id = ? ORDER BY created_at"
        )?;

        let tasks = stmt
            .query_map([project_id], |row| {
                let id: String = row.get(0)?;
                let description: String = row.get(1)?;
                let completed: bool = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                debug!(
                    "Found task: {} ({}) created at {}",
                    description, id, created_at
                );
                Ok((id, description, completed, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Successfully retrieved project tasks");
        Ok(tasks)
    }

    pub fn get_collection_tasks(
        &mut self,
        collection_id: &str,
    ) -> Result<Vec<(String, String, String, bool, i64)>, DatabaseError> {
        debug!(
            "Getting tasks for all projects in collection {}",
            collection_id
        );
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, p.name, t.description, t.completed, t.created_at
            FROM project_tasks t
            JOIN projects p ON p.id = t.project_id
            JOIN collection_projects cp ON cp.project_id = p.id
            WHERE cp.collection_id = ?
            ORDER BY cp.position, t.created_at
            "#,
        )?;

        let tasks = stmt
            .query_map([collection_id], |row| {
                let id: String = row.get(0)?;
                let project_name: String = row.get(1)?;
                let description: String = row.get(2)?;
                let completed: bool = row.get(3)?;
                let created_at: i64 = row.get(4)?;
                debug!(
                    "Found task: {} ({}) from project {} created at {}",
                    description, id, project_name, created_at
                );
                Ok((id, project_name, description, completed, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Successfully retrieved collection tasks");
        Ok(tasks)
    }

    pub fn get_task_completion_trends(
        &mut self,
        months: i32,
    ) -> Result<Vec<(i32, i32, i32, i32, f64)>, DatabaseError> {
        debug!("Getting task completion trends for last {} months", months);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                strftime('%Y', datetime(created_at, 'unixepoch')) as year,
                strftime('%m', datetime(created_at, 'unixepoch')) as month,
                COUNT(CASE WHEN completed = 1 THEN 1 END) as completed_tasks,
                COUNT(*) as total_tasks,
                CAST(COUNT(CASE WHEN completed = 1 THEN 1 END) AS REAL) / COUNT(*) as completion_rate
            FROM project_tasks
            WHERE created_at IS NOT NULL AND datetime(created_at, 'unixepoch') >= datetime('now', '-' || ? || ' months')
            GROUP BY year, month
            ORDER BY year, month
            "#
        )?;

        let trends = stmt
            .query_map([months], |row| {
                let year_str: Option<String> = row.get(0)?;
                let month_str: Option<String> = row.get(1)?;
                let year: i32 = year_str.unwrap_or_default().parse().unwrap_or(0);
                let month: i32 = month_str.unwrap_or_default().parse().unwrap_or(0);
                let completed_tasks: i32 = row.get(2)?;
                let total_tasks: i32 = row.get(3)?;
                let completion_rate: f64 = row.get(4)?;
                debug!(
                    "Found trend: {}-{:02}: {}/{} tasks ({:.2}%)",
                    year,
                    month,
                    completed_tasks,
                    total_tasks,
                    completion_rate * 100.0
                );
                Ok((year, month, completed_tasks, total_tasks, completion_rate))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Successfully retrieved task completion trends");
        Ok(trends)
    }

    // Batch Task Operations
    pub fn batch_update_task_status(
        &mut self,
        task_ids: &[String],
        completed: bool,
    ) -> Result<Vec<(String, Result<(), DatabaseError>)>, DatabaseError> {
        debug!(
            "Batch updating {} tasks completion status to {}",
            task_ids.len(),
            completed
        );
        let tx = self.conn.transaction()?;
        let mut results = Vec::new();

        for task_id in task_ids {
            let result = tx.execute(
                "UPDATE project_tasks SET completed = ? WHERE id = ?",
                params![completed, task_id],
            );

            match result {
                Ok(rows_affected) => {
                    if rows_affected > 0 {
                        debug!(
                            "Successfully updated task {} completion status to {}",
                            task_id, completed
                        );
                        results.push((task_id.clone(), Ok(())));
                    } else {
                        debug!("Task {} not found", task_id);
                        results.push((
                            task_id.clone(),
                            Err(DatabaseError::NotFound(format!(
                                "Task {} not found",
                                task_id
                            ))),
                        ));
                    }
                }
                Err(e) => {
                    debug!("Failed to update task {} completion status: {}", task_id, e);
                    results.push((task_id.clone(), Err(DatabaseError::from(e))));
                }
            }
        }

        tx.commit()?;
        debug!(
            "Batch update task status operation completed with {} results",
            results.len()
        );
        Ok(results)
    }

    pub fn batch_delete_tasks(
        &mut self,
        task_ids: &[String],
    ) -> Result<Vec<(String, Result<(), DatabaseError>)>, DatabaseError> {
        debug!("Batch deleting {} tasks", task_ids.len());
        let tx = self.conn.transaction()?;
        let mut results = Vec::new();

        for task_id in task_ids {
            let result = tx.execute("DELETE FROM project_tasks WHERE id = ?", params![task_id]);

            match result {
                Ok(rows_affected) => {
                    if rows_affected > 0 {
                        debug!("Successfully deleted task {}", task_id);
                        results.push((task_id.clone(), Ok(())));
                    } else {
                        debug!("Task {} not found", task_id);
                        results.push((
                            task_id.clone(),
                            Err(DatabaseError::NotFound(format!(
                                "Task {} not found",
                                task_id
                            ))),
                        ));
                    }
                }
                Err(e) => {
                    debug!("Failed to delete task {}: {}", task_id, e);
                    results.push((task_id.clone(), Err(DatabaseError::from(e))));
                }
            }
        }

        tx.commit()?;
        debug!(
            "Batch delete tasks operation completed with {} results",
            results.len()
        );
        Ok(results)
    }

    /// Search tasks within a specific project
    pub fn search_tasks(
        &mut self,
        project_id: &str,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        completed_only: Option<bool>,
        pending_only: Option<bool>,
    ) -> Result<(Vec<(String, String, bool, i64)>, i32), DatabaseError> {
        debug!("Searching tasks in project {} with query: {}", project_id, query);

        // Build WHERE conditions
        let mut conditions = vec!["project_id = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(project_id.to_string())];

        // Add text search condition
        conditions.push("description LIKE ?");
        params.push(Box::new(format!("%{}%", query)));

        // Add completion status filters
        if let Some(completed) = completed_only {
            if completed {
                conditions.push("completed = 1");
            }
        } else if let Some(pending) = pending_only {
            if pending {
                conditions.push("completed = 0");
            }
        }

        let where_clause = conditions.join(" AND ");

        // Get total count
        let count_query = format!("SELECT COUNT(*) FROM project_tasks WHERE {}", where_clause);
        let mut count_stmt = self.conn.prepare(&count_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let total_count: i32 = count_stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;

        // Build main query with pagination
        let main_query = format!(
            "SELECT id, description, completed, created_at FROM project_tasks WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            where_clause
        );

        // Add pagination parameters
        let limit_val = limit.unwrap_or(50);
        let offset_val = offset.unwrap_or(0);
        params.push(Box::new(limit_val));
        params.push(Box::new(offset_val));

        let mut stmt = self.conn.prepare(&main_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let tasks: Vec<(String, String, bool, i64)> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let id: String = row.get(0)?;
                let description: String = row.get(1)?;
                let completed: bool = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                Ok((id, description, completed, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Found {} tasks matching search criteria", tasks.len());
        Ok((tasks, total_count))
    }

    /// Get detailed task analytics for status bars/overviews
    pub fn get_task_analytics(&mut self, project_id: Option<&str>) -> Result<TaskAnalytics, DatabaseError> {
        debug!("Getting task statistics for project: {:?}", project_id);

        let (where_clause, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(pid) = project_id {
            ("WHERE project_id = ?".to_string(), vec![Box::new(pid.to_string())])
        } else {
            ("".to_string(), vec![])
        };

        // Get basic counts
        let query = format!("SELECT COUNT(*), COUNT(CASE WHEN completed = 1 THEN 1 END) FROM project_tasks {}", where_clause);
        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let (total_tasks, completed_tasks): (i32, i32) = stmt.query_row(param_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let pending_tasks = total_tasks - completed_tasks;
        let completion_rate = if total_tasks > 0 {
            (completed_tasks as f64 / total_tasks as f64) * 100.0
        } else {
            0.0
        };

        // Get weekly stats
        let weekly_query = if where_clause.is_empty() {
            "SELECT COUNT(*), COUNT(CASE WHEN completed = 1 THEN 1 END) FROM project_tasks WHERE created_at >= strftime('%s', 'now', '-7 days')".to_string()
        } else {
            format!("SELECT COUNT(*), COUNT(CASE WHEN completed = 1 THEN 1 END) FROM project_tasks {} AND created_at >= strftime('%s', 'now', '-7 days')", where_clause)
        };
        let mut weekly_stmt = self.conn.prepare(&weekly_query)?;
        let (tasks_created_this_week, tasks_completed_this_week): (i32, i32) = weekly_stmt.query_row(param_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        // Get monthly stats  
        let monthly_query = if where_clause.is_empty() {
            "SELECT COUNT(*), COUNT(CASE WHEN completed = 1 THEN 1 END) FROM project_tasks WHERE created_at >= strftime('%s', 'now', '-30 days')".to_string()
        } else {
            format!("SELECT COUNT(*), COUNT(CASE WHEN completed = 1 THEN 1 END) FROM project_tasks {} AND created_at >= strftime('%s', 'now', '-30 days')", where_clause)
        };
        let mut monthly_stmt = self.conn.prepare(&monthly_query)?;
        let (tasks_created_this_month, tasks_completed_this_month): (i32, i32) = monthly_stmt.query_row(param_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        // Get monthly trends (reuse existing method) - we'll do this separately to avoid borrowing issues
        drop(stmt); // Drop the statement to release the borrow
        drop(weekly_stmt);
        drop(monthly_stmt);
        
        let monthly_trends = if let Some(pid) = project_id {
            // For specific project, we need a different query
            self.get_project_task_trends(pid, 12)?
        } else {
            self.get_task_completion_trends(12)?
        };

        Ok(TaskAnalytics {
            total_tasks,
            completed_tasks,
            pending_tasks,
            completion_rate,
            tasks_created_this_week,
            tasks_completed_this_week,
            tasks_created_this_month,
            tasks_completed_this_month,
            monthly_trends,
        })
    }

    /// Get task completion trends for a specific project
    fn get_project_task_trends(
        &mut self,
        project_id: &str,
        months: i32,
    ) -> Result<Vec<(i32, i32, i32, i32, f64)>, DatabaseError> {
        debug!("Getting task completion trends for project {} over last {} months", project_id, months);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                strftime('%Y', datetime(created_at, 'unixepoch')) as year,
                strftime('%m', datetime(created_at, 'unixepoch')) as month,
                COUNT(CASE WHEN completed = 1 THEN 1 END) as completed_tasks,
                COUNT(*) as total_tasks,
                CAST(COUNT(CASE WHEN completed = 1 THEN 1 END) AS REAL) / COUNT(*) as completion_rate
            FROM project_tasks
            WHERE project_id = ? AND created_at IS NOT NULL AND datetime(created_at, 'unixepoch') >= datetime('now', '-' || ? || ' months')
            GROUP BY year, month
            ORDER BY year, month
            "#
        )?;

        let trends = stmt
            .query_map([project_id, &months.to_string()], |row| {
                let year_str: Option<String> = row.get(0)?;
                let month_str: Option<String> = row.get(1)?;
                let year: i32 = year_str.unwrap_or_default().parse().unwrap_or(0);
                let month: i32 = month_str.unwrap_or_default().parse().unwrap_or(0);
                let completed_tasks: i32 = row.get(2)?;
                let total_tasks: i32 = row.get(3)?;
                let completion_rate: f64 = row.get(4)?;
                Ok((year, month, completed_tasks, total_tasks, completion_rate))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Successfully retrieved project task completion trends");
        Ok(trends)
    }
}

// Task Analytics struct for the database layer
pub struct TaskAnalytics {
    pub total_tasks: i32,
    pub completed_tasks: i32,
    pub pending_tasks: i32,
    pub completion_rate: f64,
    pub tasks_created_this_week: i32,
    pub tasks_completed_this_week: i32,
    pub tasks_created_this_month: i32,
    pub tasks_completed_this_month: i32,
    pub monthly_trends: Vec<(i32, i32, i32, i32, f64)>, // (year, month, completed, total, rate)
}
