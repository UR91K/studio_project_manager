//! HTTP wire types for the system domain (ADR-0024).
//!
//! `StatisticsDto` mirrors `services::Statistics` field by field. The projects it
//! embeds are converted by `project_to_dto`, like any other project in a response.

use serde::{Deserialize, Serialize};

use crate::database::ProjectDatabase;
use crate::error::DatabaseError;
use crate::http::dto::collections::CollectionDto;
use crate::http::dto::projects::{project_to_dto, KeySignatureDto, ProjectDto};
use crate::services::{ScanProgress, Statistics};

#[derive(Serialize)]
pub struct SystemInfoResponse {
    pub version: String,
    pub watch_paths: Vec<String>,
    pub watcher_active: bool,
    pub uptime_seconds: i64,
    /// The config.toml in use (ADR-0076).
    pub config_path: Option<String>,
    /// The folder the log files go to; null when there is none.
    pub log_dir: Option<String>,
}

#[derive(Serialize)]
pub struct ScanStatusResponse {
    pub status: String,
    pub current_progress: Option<ScanProgressDto>,
}

#[derive(Serialize, Clone)]
pub struct ScanProgressDto {
    pub completed: u32,
    pub total: u32,
    pub progress: f32,
    pub message: String,
    pub status: String,
}

impl From<ScanProgress> for ScanProgressDto {
    fn from(p: ScanProgress) -> Self {
        Self {
            completed: p.completed,
            total: p.total,
            progress: p.progress,
            message: p.message,
            status: p.status.name().to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct WatcherEventDto {
    pub event_type: String,
    pub path: String,
    pub new_path: Option<String>,
    pub timestamp: i64,
}

#[derive(Serialize)]
pub struct AddProjectResponse {
    pub success: bool,
    pub project: Option<ProjectDto>,
    pub error_message: Option<String>,
}

#[derive(Deserialize)]
pub struct AddSingleProjectRequest {
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct AddMultipleProjectsRequest {
    pub file_paths: Vec<String>,
}

#[derive(Serialize)]
pub struct AddMultipleProjectsResponse {
    pub success: bool,
    pub projects: Vec<ProjectDto>,
    pub failed_paths: Vec<String>,
    pub error_messages: Vec<String>,
    pub total_requested: i32,
    pub successful_imports: i32,
    pub failed_imports: i32,
}

#[derive(Serialize)]
pub struct WatcherActionResponse {
    pub success: bool,
}

#[derive(Serialize)]
pub struct PluginStatisticDto {
    pub name: String,
    pub vendor: String,
    pub usage_count: i32,
}

#[derive(Serialize)]
pub struct VendorStatisticDto {
    pub vendor: String,
    pub plugin_count: i32,
    pub usage_count: i32,
}

#[derive(Serialize)]
pub struct TempoStatisticDto {
    pub tempo: f64,
    pub count: i32,
}

#[derive(Serialize)]
pub struct KeyStatisticDto {
    /// The ADR-0035 key shape; `None` for the projects with no key.
    pub key: Option<KeySignatureDto>,
    pub count: i32,
}

#[derive(Serialize)]
pub struct TimeSignatureStatisticDto {
    pub numerator: i32,
    pub denominator: i32,
    pub count: i32,
}

#[derive(Serialize)]
pub struct YearStatisticDto {
    pub year: i32,
    pub count: i32,
}

#[derive(Serialize)]
pub struct MonthStatisticDto {
    pub year: i32,
    pub month: i32,
    pub count: i32,
}

#[derive(Serialize)]
pub struct ProjectComplexityStatisticDto {
    pub project: Option<ProjectDto>,
    pub plugin_count: i32,
    pub sample_count: i32,
    pub complexity_score: i32,
}

#[derive(Serialize)]
pub struct SampleStatisticDto {
    pub name: String,
    pub path: String,
    pub usage_count: i32,
}

#[derive(Serialize)]
pub struct TagStatisticDto {
    pub name: String,
    pub usage_count: i32,
}

#[derive(Serialize)]
pub struct ActivityTrendStatisticDto {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub projects_created: i32,
    pub projects_modified: i32,
}

#[derive(Serialize)]
pub struct VersionStatisticDto {
    pub version: String,
    pub count: i32,
}

#[derive(Serialize)]
pub struct TaskCompletionTrendStatisticDto {
    pub year: i32,
    pub month: i32,
    pub completed_tasks: i32,
    pub total_tasks: i32,
    pub completion_rate: f64,
}

// The overview figures, each split into its states (ADR-0045). `total` is the sum of
// the parts. The project figures ignore the scope, since they are what it chooses
// between; every other figure counts what the projects in scope use.

#[derive(Serialize)]
pub struct ProjectCountsDto {
    pub total: i32,
    pub active: i32,
    pub archived: i32,
}

#[derive(Serialize)]
pub struct PluginCountsDto {
    pub total: i32,
    pub installed: i32,
    pub missing: i32,
    pub not_scanned: i32,
}

#[derive(Serialize)]
pub struct SampleCountsDto {
    pub total: i32,
    pub present: i32,
    pub missing: i32,
}

#[derive(Serialize)]
pub struct CollectionCountsDto {
    pub total: i32,
    pub with_projects: i32,
    pub empty: i32,
}

#[derive(Serialize)]
pub struct TagCountsDto {
    pub total: i32,
    pub in_use: i32,
    pub unused: i32,
}

#[derive(Serialize)]
pub struct TaskCountsDto {
    pub total: i32,
    pub completed: i32,
    pub pending: i32,
    /// 0 to 1.
    pub completion_rate: f64,
}

#[derive(Serialize)]
pub struct StatisticsDto {
    pub projects: ProjectCountsDto,
    pub plugins: PluginCountsDto,
    pub samples: SampleCountsDto,
    pub collections: CollectionCountsDto,
    pub tags: TagCountsDto,
    pub tasks: TaskCountsDto,
    pub top_plugins: Vec<PluginStatisticDto>,
    pub top_vendors: Vec<VendorStatisticDto>,
    pub tempo_distribution: Vec<TempoStatisticDto>,
    pub key_distribution: Vec<KeyStatisticDto>,
    pub time_signature_distribution: Vec<TimeSignatureStatisticDto>,
    pub projects_per_year: Vec<YearStatisticDto>,
    pub projects_per_month: Vec<MonthStatisticDto>,
    pub average_monthly_projects: f64,
    pub average_project_duration_seconds: f64,
    pub projects_under_40_seconds: i32,
    pub longest_project: Option<ProjectDto>,
    pub most_complex_projects: Vec<ProjectComplexityStatisticDto>,
    pub average_plugins_per_project: f64,
    pub average_samples_per_project: f64,
    pub top_samples: Vec<SampleStatisticDto>,
    pub top_tags: Vec<TagStatisticDto>,
    pub recent_activity: Vec<ActivityTrendStatisticDto>,
    pub ableton_versions: Vec<VersionStatisticDto>,
    pub average_projects_per_collection: f64,
    pub largest_collection: Option<CollectionDto>,
    pub task_completion_trends: Vec<TaskCompletionTrendStatisticDto>,
}

impl StatisticsDto {
    /// `db` is for the embedded projects, which are converted like any other.
    pub fn build(s: Statistics, db: &mut ProjectDatabase) -> Result<Self, DatabaseError> {
        let c = s.counts;
        Ok(Self {
            projects: ProjectCountsDto {
                total: c.projects_active + c.projects_archived,
                active: c.projects_active,
                archived: c.projects_archived,
            },
            plugins: PluginCountsDto {
                total: c.plugins_installed + c.plugins_missing + c.plugins_not_scanned,
                installed: c.plugins_installed,
                missing: c.plugins_missing,
                not_scanned: c.plugins_not_scanned,
            },
            samples: SampleCountsDto {
                total: c.samples_present + c.samples_missing,
                present: c.samples_present,
                missing: c.samples_missing,
            },
            collections: CollectionCountsDto {
                total: c.collections_with_projects + c.collections_empty,
                with_projects: c.collections_with_projects,
                empty: c.collections_empty,
            },
            tags: TagCountsDto {
                total: c.tags_in_use + c.tags_unused,
                in_use: c.tags_in_use,
                unused: c.tags_unused,
            },
            tasks: TaskCountsDto {
                total: c.tasks_completed + c.tasks_pending,
                completed: c.tasks_completed,
                pending: c.tasks_pending,
                completion_rate: s.task_completion_rate,
            },
            top_plugins: s
                .top_plugins
                .into_iter()
                .map(|p| PluginStatisticDto {
                    name: p.name,
                    vendor: p.vendor,
                    usage_count: p.usage_count,
                })
                .collect(),
            top_vendors: s
                .top_vendors
                .into_iter()
                .map(|v| VendorStatisticDto {
                    vendor: v.vendor,
                    plugin_count: v.plugin_count,
                    usage_count: v.usage_count,
                })
                .collect(),
            tempo_distribution: s
                .tempo_distribution
                .into_iter()
                .map(|t| TempoStatisticDto {
                    tempo: t.tempo,
                    count: t.count,
                })
                .collect(),
            key_distribution: s
                .key_distribution
                .into_iter()
                .map(|k| KeyStatisticDto {
                    key: k.key.as_deref().and_then(KeySignatureDto::from_joined),
                    count: k.count,
                })
                .collect(),
            time_signature_distribution: s
                .time_signature_distribution
                .into_iter()
                .map(|t| TimeSignatureStatisticDto {
                    numerator: t.numerator,
                    denominator: t.denominator,
                    count: t.count,
                })
                .collect(),
            projects_per_year: s
                .projects_per_year
                .into_iter()
                .map(|y| YearStatisticDto {
                    year: y.year,
                    count: y.count,
                })
                .collect(),
            projects_per_month: s
                .projects_per_month
                .into_iter()
                .map(|m| MonthStatisticDto {
                    year: m.year,
                    month: m.month,
                    count: m.count,
                })
                .collect(),
            average_monthly_projects: s.average_monthly_projects,
            average_project_duration_seconds: s.average_project_duration_seconds,
            projects_under_40_seconds: s.projects_under_40_seconds,
            longest_project: s
                .longest_project
                .map(|p| project_to_dto(p, db))
                .transpose()?,
            most_complex_projects: s
                .most_complex_projects
                .into_iter()
                .map(|c| {
                    Ok(ProjectComplexityStatisticDto {
                        project: Some(project_to_dto(c.project, db)?),
                        plugin_count: c.plugin_count,
                        sample_count: c.sample_count,
                        complexity_score: c.complexity_score,
                    })
                })
                .collect::<Result<_, DatabaseError>>()?,
            average_plugins_per_project: s.average_plugins_per_project,
            average_samples_per_project: s.average_samples_per_project,
            top_samples: s
                .top_samples
                .into_iter()
                .map(|sm| SampleStatisticDto {
                    name: sm.name,
                    path: sm.path,
                    usage_count: sm.usage_count,
                })
                .collect(),
            top_tags: s
                .top_tags
                .into_iter()
                .map(|t| TagStatisticDto {
                    name: t.name,
                    usage_count: t.usage_count,
                })
                .collect(),
            recent_activity: s
                .recent_activity
                .into_iter()
                .map(|a| ActivityTrendStatisticDto {
                    year: a.year,
                    month: a.month,
                    day: a.day,
                    projects_created: a.projects_created,
                    projects_modified: a.projects_modified,
                })
                .collect(),
            ableton_versions: s
                .ableton_versions
                .into_iter()
                .map(|v| VersionStatisticDto {
                    version: v.version,
                    count: v.count,
                })
                .collect(),
            average_projects_per_collection: s.average_projects_per_collection,
            largest_collection: s.largest_collection.map(CollectionDto::from),
            task_completion_trends: s
                .task_completion_trends
                .into_iter()
                .map(|t| TaskCompletionTrendStatisticDto {
                    year: t.year,
                    month: t.month,
                    completed_tasks: t.completed_tasks,
                    total_tasks: t.total_tasks,
                    completion_rate: t.completion_rate,
                })
                .collect(),
        })
    }
}

#[derive(Deserialize)]
pub struct ExportStatisticsQuery {
    pub format: Option<String>,
    pub scope: Option<String>,
}

#[derive(Deserialize)]
pub struct StatisticsQuery {
    pub scope: Option<String>,
}
