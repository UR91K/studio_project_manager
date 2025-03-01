use tauri::State;
use serde::Serialize;
use crate::commands::state::AppState;
use crate::live_set::LiveSet;
use crate::database::search::{SearchQuery, SearchResult};
use chrono::{DateTime, Local, TimeZone, NaiveDateTime};
use crate::models::ProjectTableInfo;

#[derive(Serialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub modified: DateTime<Local>,
    pub created: DateTime<Local>,
    pub last_scanned: DateTime<Local>,
    pub time_signature: String,
    pub key_scale: Option<String>,
    pub duration: Option<String>,
    pub ableton_version: String,
    pub plugins: Vec<String>,
    pub samples: Vec<String>,
}

impl From<LiveSet> for ProjectInfo {
    fn from(live_set: LiveSet) -> Self {
        Self {
            id: live_set.id.to_string(),
            name: live_set.name,
            filename: live_set.file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            modified: live_set.modified_time,
            created: live_set.created_time,
            last_scanned: live_set.last_parsed_timestamp,
            time_signature: format!("{}/{}", 
                live_set.time_signature.numerator,
                live_set.time_signature.denominator
            ),
            key_scale: live_set.key_signature.map(|k| 
                format!("{:?} {:?}", k.tonic, k.scale)
            ),
            duration: live_set.estimated_duration.map(|d| 
                format!("{}:{:02}", d.num_minutes(), d.num_seconds() % 60)
            ),
            ableton_version: format!("{}.{}.{}{}", 
                live_set.ableton_version.major,
                live_set.ableton_version.minor,
                live_set.ableton_version.patch,
                if live_set.ableton_version.beta { " beta" } else { "" }
            ),
            plugins: live_set.plugins.into_iter()
                .map(|p| p.name)
                .collect(),
            samples: live_set.samples.into_iter()
                .map(|s| s.name)
                .collect(),
        }
    }
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectTableInfo>, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_projects_for_table(Some(true))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_projects(
    query: String,
    state: State<'_, AppState>
) -> Result<Vec<ProjectInfo>, String> {
    // Validate query length
    if query.trim().is_empty() {
        return list_projects(state).await
            .map(|projects| projects.into_iter()
                .map(|p| ProjectInfo {
                    id: p.id,
                    name: p.name,
                    filename: p.filename,
                    modified: NaiveDateTime::parse_from_str(&p.modified, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| Local.from_local_datetime(&dt).single().unwrap())
                        .unwrap_or_else(|_| Local::now()),
                    created: NaiveDateTime::parse_from_str(&p.created, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| Local.from_local_datetime(&dt).single().unwrap())
                        .unwrap_or_else(|_| Local::now()),
                    last_scanned: NaiveDateTime::parse_from_str(&p.last_scanned, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| Local.from_local_datetime(&dt).single().unwrap())
                        .unwrap_or_else(|_| Local::now()),
                    time_signature: p.time_signature,
                    key_scale: p.key_scale,
                    duration: p.duration,
                    ableton_version: p.ableton_version,
                    plugins: p.plugins.into_iter().map(|plugin| plugin.name).collect(),
                    samples: p.samples.into_iter().map(|sample| sample.name).collect(),
                })
                .collect());
    }
    
    if query.len() > 100 {
        return Err("Search query too long (max 100 characters)".to_string());
    }
    
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let search_query = SearchQuery::parse(&query);
    db.search_fts(&search_query)
        .map_err(|e| e.to_string())
        .map(|results| results.into_iter()
            .map(|r| ProjectInfo::from(r.project))
            .collect())
} 