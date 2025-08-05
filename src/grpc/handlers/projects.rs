use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use super::super::projects::*;
use super::super::common::*;
use super::utils::convert_live_set_to_proto;
use crate::database::LiveSetDatabase;
use crate::error::DatabaseError;
use crate::LiveSet;

// MOVE FROM server.rs:
// - get_projects method (lines ~56-98)
//   Handles GetProjectsRequest with pagination
//   Calls db.get_all_projects_with_status() and converts to proto
//
// - get_project method (lines ~100-131)
//   Handles GetProjectRequest for single project
//   Calls db.get_project() and converts to proto
//
// - update_project_notes method (lines ~133-149)
//   Handles UpdateProjectNotesRequest
//   Calls db.set_project_notes()
//
// - update_project_name method (lines ~151-167)
//   Handles UpdateProjectNameRequest
//   Calls db.set_project_name()

#[derive(Clone)]
pub struct ProjectsHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl ProjectsHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_projects(
        &self,
        request: Request<GetProjectsRequest>,
    ) -> Result<Response<GetProjectsResponse>, Status> {
        debug!("GetProjects request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Use the new filtering method if any filters are specified
        let (projects, total_count) = if req.min_tempo.is_some()
            || req.max_tempo.is_some()
            || req.key_signature_tonic.is_some()
            || req.key_signature_scale.is_some()
            || req.time_signature_numerator.is_some()
            || req.time_signature_denominator.is_some()
            || req.ableton_version_major.is_some()
            || req.ableton_version_minor.is_some()
            || req.ableton_version_patch.is_some()
            || req.created_after.is_some()
            || req.created_before.is_some()
            || req.modified_after.is_some()
            || req.modified_before.is_some()
            || req.has_audio_file.is_some()
        {
            match db.get_projects_with_filters(
                req.limit,
                req.offset,
                req.sort_by,
                req.sort_desc,
                req.min_tempo,
                req.max_tempo,
                req.key_signature_tonic,
                req.key_signature_scale,
                req.time_signature_numerator,
                req.time_signature_denominator,
                req.ableton_version_major,
                req.ableton_version_minor,
                req.ableton_version_patch,
                req.created_after,
                req.created_before,
                req.modified_after,
                req.modified_before,
                req.has_audio_file,
            ) {
                Ok(result) => result,
                Err(e) => {
                    error!("Failed to get projects with filters: {:?}", e);
                    return Err(Status::new(
                        Code::Internal,
                        format!("Database error: {}", e),
                    ));
                }
            }
        } else {
            // Use the original method for backward compatibility
            match db.get_all_projects_with_status(None) {
                Ok(projects) => {
                    let total_count = projects.len() as i32;
                    let projects_iter = projects.into_iter().skip(req.offset.unwrap_or(0) as usize);
                    let projects_to_convert: Vec<LiveSet> = if let Some(limit) = req.limit {
                        projects_iter.take(limit as usize).collect()
                    } else {
                        projects_iter.collect()
                    };
                    (projects_to_convert, total_count)
                }
                Err(e) => {
                    error!("Failed to get projects: {:?}", e);
                    return Err(Status::new(
                        Code::Internal,
                        format!("Database error: {}", e),
                    ));
                }
            }
        };

        // Convert projects to proto format
        let mut proto_projects = Vec::new();
        for project in projects {
            match convert_live_set_to_proto(project, &mut *db) {
                Ok(proto_project) => proto_projects.push(proto_project),
                Err(e) => {
                    error!("Failed to convert project to proto: {:?}", e);
                    return Err(Status::internal(format!("Database error: {}", e)));
                }
            }
        }

        let response = GetProjectsResponse {
            projects: proto_projects,
            total_count,
        };
        Ok(Response::new(response))
    }

    pub async fn get_project(
        &self,
        request: Request<GetProjectRequest>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        debug!("GetProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.get_project_by_id(&req.project_id) {
            Ok(Some(project)) => match convert_live_set_to_proto(project, &mut *db) {
                Ok(proto_project) => {
                    let response = GetProjectResponse {
                        project: Some(proto_project),
                    };
                    Ok(Response::new(response))
                }
                Err(e) => {
                    error!("Failed to convert project to proto: {:?}", e);
                    Err(Status::internal(format!("Database error: {}", e)))
                }
            },
            Ok(None) => {
                let response = GetProjectResponse { project: None };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get project {}: {:?}", req.project_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn update_project_notes(
        &self,
        request: Request<UpdateProjectNotesRequest>,
    ) -> Result<Response<UpdateProjectNotesResponse>, Status> {
        debug!("UpdateProjectNotes request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.set_project_notes(&req.project_id, &req.notes) {
            Ok(()) => {
                let response = UpdateProjectNotesResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to update project notes for {}: {:?}",
                    req.project_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn update_project_name(
        &self,
        request: Request<UpdateProjectNameRequest>,
    ) -> Result<Response<UpdateProjectNameResponse>, Status> {
        debug!("UpdateProjectName request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.set_project_name(&req.project_id, &req.name) {
            Ok(()) => {
                let response = UpdateProjectNameResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to update project name for {}: {:?}",
                    req.project_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn mark_project_deleted(
        &self,
        request: Request<MarkProjectDeletedRequest>,
    ) -> Result<Response<MarkProjectDeletedResponse>, Status> {
        debug!("MarkProjectDeleted request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Parse project ID to UUID
        let project_uuid = match uuid::Uuid::parse_str(&req.project_id) {
            Ok(uuid) => uuid,
            Err(e) => {
                error!("Invalid project ID format: {}", e);
                return Err(Status::new(
                    Code::InvalidArgument,
                    "Invalid project ID format",
                ));
            }
        };

        match db.mark_project_deleted(&project_uuid) {
            Ok(()) => {
                debug!("Successfully marked project {} as deleted", req.project_id);
                let response = MarkProjectDeletedResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to mark project {} as deleted: {:?}",
                    req.project_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn reactivate_project(
        &self,
        request: Request<ReactivateProjectRequest>,
    ) -> Result<Response<ReactivateProjectResponse>, Status> {
        debug!("ReactivateProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Parse project ID to UUID
        let project_uuid = match uuid::Uuid::parse_str(&req.project_id) {
            Ok(uuid) => uuid,
            Err(e) => {
                error!("Invalid project ID format: {}", e);
                return Err(Status::new(
                    Code::InvalidArgument,
                    "Invalid project ID format",
                ));
            }
        };

        // We need to get the project's current path for reactivation
        // Since reactivate_project requires a path, we'll get it from the database
        // Use get_project_by_id_any_status to find the project regardless of active status
        match db.get_project_by_id_any_status(&req.project_id) {
            Ok(Some(project)) => match db.reactivate_project(&project_uuid, &project.file_path) {
                Ok(()) => {
                    debug!("Successfully reactivated project {}", req.project_id);
                    let response = ReactivateProjectResponse { success: true };
                    Ok(Response::new(response))
                }
                Err(e) => {
                    error!("Failed to reactivate project {}: {:?}", req.project_id, e);
                    Err(Status::new(
                        Code::Internal,
                        format!("Database error: {}", e),
                    ))
                }
            },
            Ok(None) => {
                error!("Project {} not found", req.project_id);
                Err(Status::new(Code::NotFound, "Project not found"))
            }
            Err(e) => {
                error!("Failed to get project {}: {:?}", req.project_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_projects_by_deletion_status(
        &self,
        request: Request<GetProjectsByDeletionStatusRequest>,
    ) -> Result<Response<GetProjectsByDeletionStatusResponse>, Status> {
        debug!("GetProjectsByDeletionStatus request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Convert is_deleted to is_active (inverse)
        let is_active = Some(!req.is_deleted);

        match db.get_all_projects_with_status(is_active) {
            Ok(projects) => {
                let total_count = projects.len() as i32;
                let projects_iter = projects.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let mut proto_projects = Vec::new();

                let projects_to_convert: Vec<LiveSet> = if let Some(limit) = req.limit {
                    projects_iter.take(limit as usize).collect()
                } else {
                    projects_iter.collect()
                };

                for project in projects_to_convert {
                    match convert_live_set_to_proto(project, &mut *db) {
                        Ok(proto_project) => proto_projects.push(proto_project),
                        Err(e) => {
                            error!("Failed to convert project to proto: {:?}", e);
                            return Err(Status::internal(format!("Database error: {}", e)));
                        }
                    }
                }

                let response = GetProjectsByDeletionStatusResponse {
                    projects: proto_projects,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get projects by deletion status: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn permanently_delete_project(
        &self,
        request: Request<PermanentlyDeleteProjectRequest>,
    ) -> Result<Response<PermanentlyDeleteProjectResponse>, Status> {
        debug!("PermanentlyDeleteProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Parse project ID to UUID
        let project_uuid = match uuid::Uuid::parse_str(&req.project_id) {
            Ok(uuid) => uuid,
            Err(e) => {
                error!("Invalid project ID format: {}", e);
                return Err(Status::new(
                    Code::InvalidArgument,
                    "Invalid project ID format",
                ));
            }
        };

        match db.permanently_delete_project(&project_uuid) {
            Ok(()) => {
                debug!(
                    "Successfully permanently deleted project {}",
                    req.project_id
                );
                let response = PermanentlyDeleteProjectResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => match e {
                DatabaseError::InvalidOperation(msg)
                    if msg == "Cannot permanently delete an active project" =>
                {
                    debug!(
                        "Cannot permanently delete active project {}",
                        req.project_id
                    );
                    let response = PermanentlyDeleteProjectResponse { success: false };
                    Ok(Response::new(response))
                }
                _ => {
                    error!(
                        "Failed to permanently delete project {}: {:?}",
                        req.project_id, e
                    );
                    Err(Status::new(
                        Code::Internal,
                        format!("Database error: {}", e),
                    ))
                }
            },
        }
    }

    // Batch Project Operations
    pub async fn batch_mark_projects_as_archived(
        &self,
        request: Request<BatchMarkProjectsAsArchivedRequest>,
    ) -> Result<Response<BatchMarkProjectsAsArchivedResponse>, Status> {
        debug!("BatchMarkProjectsAsArchived request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.batch_mark_projects_archived(&req.project_ids, req.archived) {
            Ok(results) => {
                let (successful_count, failed_count) =
                    results
                        .iter()
                        .fold((0, 0), |(success, fail), (_, result)| match result {
                            Ok(_) => (success + 1, fail),
                            Err(_) => (success, fail + 1),
                        });

                let batch_results = results
                    .into_iter()
                    .map(|(id, result)| BatchOperationResult {
                        id,
                        success: result.is_ok(),
                        error_message: result.err().map(|e| e.to_string()),
                    })
                    .collect();

                let response = BatchMarkProjectsAsArchivedResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                };

                debug!(
                    "Batch archive operation completed: {} successful, {} failed",
                    successful_count, failed_count
                );
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to batch mark projects as archived: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn batch_delete_projects(
        &self,
        request: Request<BatchDeleteProjectsRequest>,
    ) -> Result<Response<BatchDeleteProjectsResponse>, Status> {
        debug!("BatchDeleteProjects request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.batch_delete_projects(&req.project_ids) {
            Ok(results) => {
                let (successful_count, failed_count) =
                    results
                        .iter()
                        .fold((0, 0), |(success, fail), (_, result)| match result {
                            Ok(_) => (success + 1, fail),
                            Err(_) => (success, fail + 1),
                        });

                let batch_results = results
                    .into_iter()
                    .map(|(id, result)| BatchOperationResult {
                        id,
                        success: result.is_ok(),
                        error_message: result.err().map(|e| e.to_string()),
                    })
                    .collect();

                let response = BatchDeleteProjectsResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                };

                debug!(
                    "Batch delete operation completed: {} successful, {} failed",
                    successful_count, failed_count
                );
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to batch delete projects: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_project_statistics(
        &self,
        request: Request<GetProjectStatisticsRequest>,
    ) -> Result<Response<GetProjectStatisticsResponse>, Status> {
        debug!("GetProjectStatistics request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_project_statistics(
            req.min_tempo,
            req.max_tempo,
            req.key_signature_tonic,
            req.key_signature_scale,
            req.time_signature_numerator,
            req.time_signature_denominator,
            req.ableton_version_major,
            req.ableton_version_minor,
            req.ableton_version_patch,
            req.created_after,
            req.created_before,
            req.has_audio_file,
        ) {
            Ok(stats) => {
                // Convert statistics to proto format
                let tempo_distribution = stats
                    .tempo_distribution
                    .into_iter()
                    .map(|(range, count)| TempoRangeStatistic { range, count })
                    .collect();

                let key_signature_distribution = stats
                    .key_signature_distribution
                    .into_iter()
                    .map(|(key_signature, count)| KeySignatureStatistic { key_signature, count })
                    .collect();

                let time_signature_distribution = stats
                    .time_signature_distribution
                    .into_iter()
                    .map(|(numerator, denominator, count)| TimeSignatureStatistic {
                        numerator,
                        denominator,
                        count,
                    })
                    .collect();

                let ableton_version_distribution = stats
                    .ableton_version_distribution
                    .into_iter()
                    .map(|(version, count)| AbletonVersionStatistic { version, count })
                    .collect();

                let projects_per_year = stats
                    .projects_per_year
                    .into_iter()
                    .map(|(year, count)| YearStatistic { year, count })
                    .collect();

                let projects_per_month = stats
                    .projects_per_month
                    .into_iter()
                    .map(|(year, month, count)| MonthStatistic { year, month, count })
                    .collect();

                let most_complex_projects = stats
                    .most_complex_projects
                    .into_iter()
                    .map(|(project_id, project_name, plugin_count, sample_count, tag_count, complexity_score)| {
                        ProjectComplexityStatistic {
                            project_id,
                            project_name,
                            plugin_count,
                            sample_count,
                            tag_count,
                            complexity_score,
                        }
                    })
                    .collect();

                let response = GetProjectStatisticsResponse {
                    total_projects: stats.total_projects,
                    projects_with_audio_files: stats.projects_with_audio_files,
                    projects_without_audio_files: stats.projects_without_audio_files,
                    average_tempo: stats.average_tempo,
                    min_tempo: stats.min_tempo,
                    max_tempo: stats.max_tempo,
                    tempo_distribution,
                    key_signature_distribution,
                    time_signature_distribution,
                    ableton_version_distribution,
                    average_duration_seconds: stats.average_duration_seconds,
                    min_duration_seconds: stats.min_duration_seconds,
                    max_duration_seconds: stats.max_duration_seconds,
                    average_plugins_per_project: stats.average_plugins_per_project,
                    average_samples_per_project: stats.average_samples_per_project,
                    average_tags_per_project: stats.average_tags_per_project,
                    projects_per_year,
                    projects_per_month,
                    most_complex_projects,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get project statistics: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn rescan_project(
        &self,
        request: Request<RescanProjectRequest>,
    ) -> Result<Response<RescanProjectResponse>, Status> {
        debug!("RescanProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.rescan_project(&req.project_id, req.force_rescan.unwrap_or(false)) {
            Ok(result) => {
                let updated_project = if let Some(project) = result.updated_project {
                    match convert_live_set_to_proto(project, &mut *db) {
                        Ok(proto_project) => Some(proto_project),
                        Err(e) => {
                            error!("Failed to convert updated project to proto: {:?}", e);
                            return Err(Status::internal(format!("Database error: {}", e)));
                        }
                    }
                } else {
                    None
                };

                let response = RescanProjectResponse {
                    success: result.success,
                    updated_project,
                    error_message: result.error_message,
                    was_updated: result.was_updated,
                    scan_summary: result.scan_summary,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to rescan project {}: {:?}", req.project_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }
}
