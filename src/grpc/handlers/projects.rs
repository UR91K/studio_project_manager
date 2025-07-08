use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Code};
use log::{debug, error};

use crate::database::LiveSetDatabase;
use crate::LiveSet;
use super::super::proto::*;
use super::utils::convert_live_set_to_proto;

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
        match db.get_all_projects_with_status(None) {
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
                
                let response = GetProjectsResponse {
                    projects: proto_projects,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get projects: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn get_project(
        &self,
        request: Request<GetProjectRequest>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        debug!("GetProject request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.get_project(&req.project_id) {
            Ok(Some(project)) => {
                match convert_live_set_to_proto(project, &mut *db) {
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
                }
            }
            Ok(None) => {
                let response = GetProjectResponse { project: None };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get project {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                error!("Failed to update project notes for {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                error!("Failed to update project name for {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }
} 