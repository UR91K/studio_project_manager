use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use crate::database::LiveSetDatabase;
use super::super::tags::*;
use super::super::common::*;

// MOVE FROM server.rs:
// - get_tags method (lines ~448-470)
//   Handles GetTagsRequest
//   Calls db.list_tags() and converts to proto Tag format
//
// - create_tag method (lines ~472-504)
//   Handles CreateTagRequest
//   Calls db.add_tag() and db.get_tag_by_id() to return created tag
//
// - tag_project method (lines ~506-522)
//   Handles TagProjectRequest
//   Calls db.tag_project() to associate tag with project
//
// - untag_project method (lines ~524-540)
//   Handles UntagProjectRequest
//   Calls db.untag_project() to remove tag from project

#[derive(Clone)]
pub struct TagsHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl TagsHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_tags(
        &self,
        _request: Request<GetTagsRequest>,
    ) -> Result<Response<GetTagsResponse>, Status> {
        debug!("GetTags request");

        let mut db = self.db.lock().await;
        match db.list_tags() {
            Ok(tags_data) => {
                let tags = tags_data
                    .into_iter()
                    .map(|(id, name, created_at)| Tag {
                        id,
                        name,
                        created_at,
                    })
                    .collect();

                let response = GetTagsResponse { tags };
                debug!("Successfully retrieved {} tags", response.tags.len());
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get tags: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to get tags: {}", e),
                ))
            }
        }
    }

    pub async fn create_tag(
        &self,
        request: Request<CreateTagRequest>,
    ) -> Result<Response<CreateTagResponse>, Status> {
        debug!("CreateTag request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.add_tag(&req.name) {
            Ok(tag_id) => {
                // Get the created tag details to return in response
                match db.get_tag_by_id(&tag_id) {
                    Ok(Some((id, name, created_at))) => {
                        let tag = Tag {
                            id,
                            name,
                            created_at,
                        };

                        let response = CreateTagResponse { tag: Some(tag) };
                        debug!("Successfully created tag: {}", req.name);
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Created tag not found after creation");
                        Err(Status::new(
                            Code::Internal,
                            "Created tag not found after creation".to_string(),
                        ))
                    }
                    Err(e) => {
                        error!("Failed to retrieve created tag: {}", e);
                        Err(Status::new(
                            Code::Internal,
                            format!("Failed to retrieve created tag: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create tag: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to create tag: {}", e),
                ))
            }
        }
    }

    pub async fn update_tag(
        &self,
        request: Request<UpdateTagRequest>,
    ) -> Result<Response<UpdateTagResponse>, Status> {
        debug!("UpdateTag request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_tag(&req.tag_id, &req.name) {
            Ok(()) => {
                // Get the updated tag details to return in response
                match db.get_tag_by_id(&req.tag_id) {
                    Ok(Some((id, name, created_at))) => {
                        let tag = Tag {
                            id,
                            name,
                            created_at,
                        };

                        let response = UpdateTagResponse { tag: Some(tag) };
                        debug!("Successfully updated tag: {}", req.tag_id);
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Tag {} not found after update", req.tag_id);
                        Err(Status::new(Code::NotFound, "Tag not found"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve updated tag {}: {:?}", req.tag_id, e);
                        Err(Status::new(
                            Code::Internal,
                            format!("Database error: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to update tag '{}': {:?}", req.tag_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn delete_tag(
        &self,
        request: Request<DeleteTagRequest>,
    ) -> Result<Response<DeleteTagResponse>, Status> {
        debug!("DeleteTag request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.remove_tag(&req.tag_id) {
            Ok(()) => {
                debug!("Successfully deleted tag: {}", req.tag_id);
                let response = DeleteTagResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to delete tag '{}': {:?}", req.tag_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn tag_project(
        &self,
        request: Request<TagProjectRequest>,
    ) -> Result<Response<TagProjectResponse>, Status> {
        debug!("TagProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.tag_project(&req.project_id, &req.tag_id) {
            Ok(()) => {
                debug!(
                    "Successfully tagged project {} with tag {}",
                    req.project_id, req.tag_id
                );
                let response = TagProjectResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to tag project: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to tag project: {}", e),
                ))
            }
        }
    }

    pub async fn untag_project(
        &self,
        request: Request<UntagProjectRequest>,
    ) -> Result<Response<UntagProjectResponse>, Status> {
        debug!("UntagProject request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.untag_project(&req.project_id, &req.tag_id) {
            Ok(()) => {
                debug!(
                    "Successfully untagged project {} from tag {}",
                    req.project_id, req.tag_id
                );
                let response = UntagProjectResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to untag project: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to untag project: {}", e),
                ))
            }
        }
    }

    // Batch Tag Operations
    pub async fn batch_tag_projects(
        &self,
        request: Request<BatchTagProjectsRequest>,
    ) -> Result<Response<BatchTagProjectsResponse>, Status> {
        debug!("BatchTagProjects request: {:?}", request);
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        match db.batch_tag_projects(&req.project_ids, &req.tag_ids) {
            Ok(results) => {
                let (successful_count, failed_count) = results.iter().fold(
                    (0, 0),
                    |(s, f), (_, r)| {
                        if r.is_ok() {
                            (s + 1, f)
                        } else {
                            (s, f + 1)
                        }
                    },
                );
                let batch_results = results
                    .into_iter()
                    .map(|(id, result)| BatchOperationResult {
                        id,
                        success: result.is_ok(),
                        error_message: result.err().map(|e| e.to_string()),
                    })
                    .collect();
                Ok(Response::new(BatchTagProjectsResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                }))
            }
            Err(e) => Err(Status::internal(format!("Database error: {}", e))),
        }
    }

    pub async fn batch_untag_projects(
        &self,
        request: Request<BatchUntagProjectsRequest>,
    ) -> Result<Response<BatchUntagProjectsResponse>, Status> {
        debug!("BatchUntagProjects request: {:?}", request);
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        match db.batch_untag_projects(&req.project_ids, &req.tag_ids) {
            Ok(results) => {
                let (successful_count, failed_count) = results.iter().fold(
                    (0, 0),
                    |(s, f), (_, r)| {
                        if r.is_ok() {
                            (s + 1, f)
                        } else {
                            (s, f + 1)
                        }
                    },
                );
                let batch_results = results
                    .into_iter()
                    .map(|(id, result)| BatchOperationResult {
                        id,
                        success: result.is_ok(),
                        error_message: result.err().map(|e| e.to_string()),
                    })
                    .collect();
                Ok(Response::new(BatchUntagProjectsResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                }))
            }
            Err(e) => Err(Status::internal(format!("Database error: {}", e))),
        }
    }
}
