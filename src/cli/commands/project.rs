use crate::cli::commands::{CliCommand, CliContext};
use crate::cli::ProjectCommands;
use crate::cli::CliError;
use crate::cli::output::{OutputFormatter, TableDisplay};
use comfy_table::Table;
use serde::Serialize;
use uuid::Uuid;
use chrono::Utc;

#[async_trait::async_trait]
impl CliCommand for ProjectCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<(), CliError> {
        match self {
            ProjectCommands::List { deleted, limit, offset } => {
                self.list_projects(ctx, *deleted, *limit, *offset).await
            }
            ProjectCommands::Show { id } => self.show_project(ctx, id).await,
            ProjectCommands::Update { id, name, notes } => {
                self.update_project(ctx, id, name.as_deref(), notes.as_deref()).await
            }
            ProjectCommands::Delete { id } => self.delete_project(ctx, id).await,
            ProjectCommands::Restore { id } => self.restore_project(ctx, id).await,
            ProjectCommands::Rescan { id } => self.rescan_project(ctx, id).await,
            ProjectCommands::Stats => self.show_project_stats(ctx).await,
        }
    }
}

impl ProjectCommands {
    async fn list_projects(&self, ctx: &CliContext, deleted: bool, limit: usize, offset: usize) -> Result<(), CliError> {
        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);

        let db = ctx.db.lock().await;
        let status = if deleted { Some(false) } else { Some(true) };
        let projects = db.get_all_projects_with_status(status)?;

        // Simple pagination
        let start = offset.min(projects.len());
        let end = (start + limit).min(projects.len());
        let slice = &projects[start..end];

        let rows: Vec<ProjectRow> = slice
            .iter()
            .map(|p| ProjectRow {
                id: p.id.to_string(),
                name: p.name.clone(),
                path: p.file_path.display().to_string(),
                tempo: p.tempo,
                key: p
                    .key_signature
                    .as_ref()
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| "".to_string()),
                time_signature: format!("{}/{}", p.time_signature.numerator, p.time_signature.denominator),
            })
            .collect();

        let data = ProjectsList { total: projects.len(), displayed: rows };
        formatter.print(&data)
    }

    async fn show_project(&self, ctx: &CliContext, id: &str) -> Result<(), CliError> {
        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
        let mut db = ctx.db.lock().await;
        match db.get_project_by_id(id)? {
            Some(p) => {
                let details = ProjectDetails::from_live_set(&p);
                formatter.print(&details)
            }
            None => {
                formatter.print_message(&format!("Project not found: {}", id), crate::cli::output::MessageType::Warning);
                Ok(())
            }
        }
    }

    async fn update_project(&self, ctx: &CliContext, id: &str, name: Option<&str>, notes: Option<&str>) -> Result<(), CliError> {
        if name.is_none() && notes.is_none() {
            return Ok(());
        }

        let db = ctx.db.lock().await;
        let ts = Utc::now().timestamp();
        match (name, notes) {
            (Some(n), Some(s)) => {
                db.conn.execute(
                    "UPDATE projects SET name = ?, notes = ?, modified_at = ? WHERE id = ?",
                    rusqlite::params![n, s, ts, id],
                )?;
            }
            (Some(n), None) => {
                db.conn.execute(
                    "UPDATE projects SET name = ?, modified_at = ? WHERE id = ?",
                    rusqlite::params![n, ts, id],
                )?;
            }
            (None, Some(s)) => {
                db.conn.execute(
                    "UPDATE projects SET notes = ?, modified_at = ? WHERE id = ?",
                    rusqlite::params![s, ts, id],
                )?;
            }
            (None, None) => {}
        }

        // Show updated project summary
        self.show_project(ctx, id).await
    }

    async fn delete_project(&self, ctx: &CliContext, id: &str) -> Result<(), CliError> {
        let mut db = ctx.db.lock().await;
        let uuid = Uuid::parse_str(id).map_err(|e| -> CliError { e.into() })?;
        db.mark_project_deleted(&uuid)?;
        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
        formatter.print_message(&format!("Project {} marked as deleted", id), crate::cli::output::MessageType::Success);
        Ok(())
    }

    async fn restore_project(&self, ctx: &CliContext, id: &str) -> Result<(), CliError> {
        let mut db = ctx.db.lock().await;
        let uuid = Uuid::parse_str(id).map_err(|e| -> CliError { e.into() })?;

        // Get stored path
        let project_any = db.get_project_by_id_any_status(id)?;
        let project = match project_any {
            Some(p) => p,
            None => {
                let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
                formatter.print_message(&format!("Project not found: {}", id), crate::cli::output::MessageType::Warning);
                return Ok(());
            }
        };

        db.reactivate_project(&uuid, &project.file_path)?;

        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
        formatter.print_message(&format!("Project {} restored", id), crate::cli::output::MessageType::Success);
        Ok(())
    }

    async fn rescan_project(&self, ctx: &CliContext, id: &str) -> Result<(), CliError> {
        let mut db = ctx.db.lock().await;
        let result = db.rescan_project(id, false)?;
        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
        if result.success {
            formatter.print_message(&result.scan_summary, crate::cli::output::MessageType::Success);
        } else {
            formatter.print_message(&format!("Rescan failed: {}", result.error_message.unwrap_or_else(|| "Unknown error".to_string())), crate::cli::output::MessageType::Error);
        }
        Ok(())
    }

    async fn show_project_stats(&self, ctx: &CliContext) -> Result<(), CliError> {
        let db = ctx.db.lock().await;
        let stats = db.get_project_statistics(
            None, None, None, None, None, None, None, None, None, None, None, None,
        )?;

        let display = ProjectStatisticsDisplay::from_stats(&stats);
        let formatter = OutputFormatter::new(ctx.output_format.clone(), ctx.no_color);
        formatter.print(&display)
    }
}

// Display types

#[derive(Serialize)]
struct ProjectRow {
    id: String,
    name: String,
    path: String,
    tempo: f64,
    key: String,
    time_signature: String,
}

#[derive(Serialize)]
struct ProjectsList {
    total: usize,
    displayed: Vec<ProjectRow>,
}

impl TableDisplay for ProjectsList {
    fn to_table(&self) -> Table {
        let mut table = Table::new();
        table.set_header(vec!["ID", "Name", "Path", "Tempo", "Key", "Time Sig"]);
        for row in &self.displayed {
            table.add_row(vec![
                row.id.clone(),
                row.name.clone(),
                row.path.clone(),
                format!("{:.1}", row.tempo),
                row.key.clone(),
                row.time_signature.clone(),
            ]);
        }
        table
    }

    fn to_csv<W: std::io::Write>(&self, writer: &mut csv::Writer<W>) -> Result<(), CliError> {
        writer.write_record(["id", "name", "path", "tempo", "key", "time_signature"]).map_err(|e| -> CliError { e.into() })?;
        for row in &self.displayed {
            writer
                .write_record([
                    row.id.as_str(),
                    row.name.as_str(),
                    row.path.as_str(),
                    &format!("{:.1}", row.tempo),
                    row.key.as_str(),
                    row.time_signature.as_str(),
                ])
                .map_err(|e| -> CliError { e.into() })?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct ProjectDetails {
    id: String,
    name: String,
    path: String,
    tempo: f64,
    time_signature: String,
    key: String,
    ableton_version: String,
    created_at: String,
    modified_at: String,
    plugins: usize,
    samples: usize,
}

impl ProjectDetails {
    fn from_live_set(p: &crate::LiveSet) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            path: p.file_path.display().to_string(),
            tempo: p.tempo,
            time_signature: format!("{}/{}", p.time_signature.numerator, p.time_signature.denominator),
            key: p.key_signature.as_ref().map(|k| k.to_string()).unwrap_or_else(|| "".to_string()),
            ableton_version: p.ableton_version.to_string(),
            created_at: p.created_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            modified_at: p.modified_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            plugins: p.plugins.len(),
            samples: p.samples.len(),
        }
    }
}

impl TableDisplay for ProjectDetails {
    fn to_table(&self) -> Table {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["ID".to_string(), self.id.clone()]);
        table.add_row(vec!["Name".to_string(), self.name.clone()]);
        table.add_row(vec!["Path".to_string(), self.path.clone()]);
        table.add_row(vec!["Tempo".to_string(), format!("{:.1}", self.tempo)]);
        table.add_row(vec!["Time Signature".to_string(), self.time_signature.clone()]);
        table.add_row(vec!["Key".to_string(), self.key.clone()]);
        table.add_row(vec!["Ableton Version".to_string(), self.ableton_version.clone()]);
        table.add_row(vec!["Created".to_string(), self.created_at.clone()]);
        table.add_row(vec!["Modified".to_string(), self.modified_at.clone()]);
        table.add_row(vec!["Plugins".to_string(), self.plugins.to_string()]);
        table.add_row(vec!["Samples".to_string(), self.samples.to_string()]);
        table
    }

    fn to_csv<W: std::io::Write>(&self, writer: &mut csv::Writer<W>) -> Result<(), CliError> {
        writer.write_record(["field", "value"]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["id", &self.id]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["name", &self.name]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["path", &self.path]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["tempo", &format!("{:.1}", self.tempo)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["time_signature", &self.time_signature]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["key", &self.key]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["ableton_version", &self.ableton_version]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["created_at", &self.created_at]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["modified_at", &self.modified_at]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["plugins", &self.plugins.to_string()]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["samples", &self.samples.to_string()]).map_err(|e| -> CliError { e.into() })?;
        Ok(())
    }
}

#[derive(Serialize)]
struct ProjectStatisticsDisplay {
    total_projects: i32,
    projects_with_audio_files: i32,
    projects_without_audio_files: i32,
    average_tempo: f64,
    min_tempo: f64,
    max_tempo: f64,
    average_duration_seconds: f64,
    min_duration_seconds: f64,
    max_duration_seconds: f64,
    average_plugins_per_project: f64,
    average_samples_per_project: f64,
    average_tags_per_project: f64,
}

impl ProjectStatisticsDisplay {
    fn from_stats(s: &crate::database::stats::ProjectStatistics) -> Self {
        Self {
            total_projects: s.total_projects,
            projects_with_audio_files: s.projects_with_audio_files,
            projects_without_audio_files: s.projects_without_audio_files,
            average_tempo: s.average_tempo,
            min_tempo: s.min_tempo,
            max_tempo: s.max_tempo,
            average_duration_seconds: s.average_duration_seconds,
            min_duration_seconds: s.min_duration_seconds,
            max_duration_seconds: s.max_duration_seconds,
            average_plugins_per_project: s.average_plugins_per_project,
            average_samples_per_project: s.average_samples_per_project,
            average_tags_per_project: s.average_tags_per_project,
        }
    }
}

impl TableDisplay for ProjectStatisticsDisplay {
    fn to_table(&self) -> Table {
        let mut table = Table::new();
        table.set_header(vec!["Metric", "Value"]);
        table.add_row(vec!["Total Projects".to_string(), self.total_projects.to_string()]);
        table.add_row(vec!["Projects with Audio".to_string(), self.projects_with_audio_files.to_string()]);
        table.add_row(vec!["Projects without Audio".to_string(), self.projects_without_audio_files.to_string()]);
        table.add_row(vec!["Average Tempo".to_string(), format!("{:.2}", self.average_tempo)]);
        table.add_row(vec!["Min Tempo".to_string(), format!("{:.2}", self.min_tempo)]);
        table.add_row(vec!["Max Tempo".to_string(), format!("{:.2}", self.max_tempo)]);
        table.add_row(vec!["Average Duration (s)".to_string(), format!("{:.2}", self.average_duration_seconds)]);
        table.add_row(vec!["Min Duration (s)".to_string(), format!("{:.2}", self.min_duration_seconds)]);
        table.add_row(vec!["Max Duration (s)".to_string(), format!("{:.2}", self.max_duration_seconds)]);
        table.add_row(vec!["Average Plugins/Project".to_string(), format!("{:.2}", self.average_plugins_per_project)]);
        table.add_row(vec!["Average Samples/Project".to_string(), format!("{:.2}", self.average_samples_per_project)]);
        table.add_row(vec!["Average Tags/Project".to_string(), format!("{:.2}", self.average_tags_per_project)]);
        table
    }

    fn to_csv<W: std::io::Write>(&self, writer: &mut csv::Writer<W>) -> Result<(), CliError> {
        writer.write_record(["metric", "value"]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["total_projects", &self.total_projects.to_string()]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["projects_with_audio_files", &self.projects_with_audio_files.to_string()]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["projects_without_audio_files", &self.projects_without_audio_files.to_string()]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["average_tempo", &format!("{:.2}", self.average_tempo)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["min_tempo", &format!("{:.2}", self.min_tempo)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["max_tempo", &format!("{:.2}", self.max_tempo)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["average_duration_seconds", &format!("{:.2}", self.average_duration_seconds)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["min_duration_seconds", &format!("{:.2}", self.min_duration_seconds)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["max_duration_seconds", &format!("{:.2}", self.max_duration_seconds)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["average_plugins_per_project", &format!("{:.2}", self.average_plugins_per_project)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["average_samples_per_project", &format!("{:.2}", self.average_samples_per_project)]).map_err(|e| -> CliError { e.into() })?;
        writer.write_record(["average_tags_per_project", &format!("{:.2}", self.average_tags_per_project)]).map_err(|e| -> CliError { e.into() })?;
        Ok(())
    }
}
