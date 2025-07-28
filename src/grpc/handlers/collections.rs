use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use super::super::collections::*;
use super::super::common::*;
use crate::database::LiveSetDatabase;

// MOVE FROM server.rs:
// - get_collections method (lines ~300-342)
//   Handles GetCollectionsRequest
//   Calls db.list_collections() and db.get_collection_by_id() for details
//
// - create_collection method (lines ~344-376)
//   Handles CreateCollectionRequest
//   Calls db.create_collection() and returns created collection
//
// - update_collection method (lines ~378-410)
//   Handles UpdateCollectionRequest
//   Calls db.update_collection() and returns updated collection
//
// - add_project_to_collection method (lines ~412-428)
//   Handles AddProjectToCollectionRequest
//   Calls db.add_project_to_collection()
//
// - remove_project_from_collection method (lines ~430-446)
//   Handles RemoveProjectFromCollectionRequest
//   Calls db.remove_project_from_collection()

#[derive(Clone)]
pub struct CollectionsHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl CollectionsHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_collections(
        &self,
        request: Request<GetCollectionsRequest>,
    ) -> Result<Response<GetCollectionsResponse>, Status> {
        debug!("GetCollections request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.list_collections(
            req.limit,
            req.offset,
            req.sort_by,
            req.sort_desc,
        ) {
            Ok((collections_data, total_count)) => {
                let mut collections = Vec::new();

                for (id, name, description) in collections_data {
                    // Get the full collection details including project IDs
                    match db.get_collection_by_id(&id) {
                        Ok(Some((
                            _,
                            _,
                            _,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                        ))) => {
                            // Get collection statistics
                            let (total_duration_seconds, project_count) =
                                db.get_collection_statistics(&id).unwrap_or((None, 0));

                            collections.push(Collection {
                                id: id.clone(),
                                name: name.clone(),
                                description,
                                notes,
                                created_at,
                                modified_at,
                                project_ids,
                                cover_art_id,
                                total_duration_seconds,
                                project_count,
                            });
                        }
                        Ok(None) => {
                            // Collection was deleted between list_collections and get_collection_by_id
                            debug!("Collection {} not found during detailed lookup", id);
                        }
                        Err(e) => {
                            error!("Failed to get collection details for {}: {:?}", id, e);
                            return Err(Status::new(
                                Code::Internal,
                                format!("Database error: {}", e),
                            ));
                        }
                    }
                }

                let response = GetCollectionsResponse { 
                    collections,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get collections: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_collection(
        &self,
        request: Request<GetCollectionRequest>,
    ) -> Result<Response<GetCollectionResponse>, Status> {
        debug!("GetCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.get_collection_by_id(&req.collection_id) {
            Ok(Some((
                id,
                name,
                description,
                notes,
                created_at,
                modified_at,
                project_ids,
                cover_art_id,
            ))) => {
                // Get collection statistics
                let (total_duration_seconds, project_count) =
                    db.get_collection_statistics(&id).unwrap_or((None, 0));

                let collection = Collection {
                    id,
                    name,
                    description,
                    notes,
                    created_at,
                    modified_at,
                    project_ids,
                    cover_art_id,
                    total_duration_seconds,
                    project_count,
                };

                let response = GetCollectionResponse {
                    collection: Some(collection),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                debug!("Collection {} not found", req.collection_id);
                let response = GetCollectionResponse { collection: None };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get collection {}: {:?}", req.collection_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> Result<Response<CreateCollectionResponse>, Status> {
        debug!("CreateCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.create_collection(&req.name, req.description.as_deref(), req.notes.as_deref()) {
            Ok(collection_id) => {
                // Get the created collection details to return in response
                match db.get_collection_by_id(&collection_id) {
                    Ok(Some((
                        id,
                        name,
                        description,
                        notes,
                        created_at,
                        modified_at,
                        project_ids,
                        cover_art_id,
                    ))) => {
                        // Get collection statistics
                        let (total_duration_seconds, project_count) =
                            db.get_collection_statistics(&id).unwrap_or((None, 0));

                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                            total_duration_seconds,
                            project_count,
                        };

                        let response = CreateCollectionResponse {
                            collection: Some(collection),
                        };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} was created but not found", collection_id);
                        Err(Status::new(Code::Internal, "Collection creation failed"))
                    }
                    Err(e) => {
                        error!(
                            "Failed to retrieve created collection {}: {:?}",
                            collection_id, e
                        );
                        Err(Status::new(
                            Code::Internal,
                            format!("Database error: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create collection '{}': {:?}", req.name, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn update_collection(
        &self,
        request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<UpdateCollectionResponse>, Status> {
        debug!("UpdateCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_collection(
            &req.collection_id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.notes.as_deref(),
        ) {
            Ok(()) => {
                // Get the updated collection details to return in response
                match db.get_collection_by_id(&req.collection_id) {
                    Ok(Some((
                        id,
                        name,
                        description,
                        notes,
                        created_at,
                        modified_at,
                        project_ids,
                        cover_art_id,
                    ))) => {
                        // Get collection statistics
                        let (total_duration_seconds, project_count) =
                            db.get_collection_statistics(&id).unwrap_or((None, 0));

                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                            total_duration_seconds,
                            project_count,
                        };

                        let response = UpdateCollectionResponse {
                            collection: Some(collection),
                        };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} not found after update", req.collection_id);
                        Err(Status::new(Code::NotFound, "Collection not found"))
                    }
                    Err(e) => {
                        error!(
                            "Failed to retrieve updated collection {}: {:?}",
                            req.collection_id, e
                        );
                        Err(Status::new(
                            Code::Internal,
                            format!("Database error: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to update collection '{}': {:?}",
                    req.collection_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn delete_collection(
        &self,
        request: Request<DeleteCollectionRequest>,
    ) -> Result<Response<DeleteCollectionResponse>, Status> {
        debug!("DeleteCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.delete_collection(&req.collection_id) {
            Ok(()) => {
                debug!("Successfully deleted collection: {}", req.collection_id);
                let response = DeleteCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to delete collection '{}': {:?}",
                    req.collection_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn add_project_to_collection(
        &self,
        request: Request<AddProjectToCollectionRequest>,
    ) -> Result<Response<AddProjectToCollectionResponse>, Status> {
        debug!("AddProjectToCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.add_project_to_collection(&req.collection_id, &req.project_id) {
            Ok(()) => {
                debug!(
                    "Successfully added project {} to collection {}",
                    req.project_id, req.collection_id
                );
                let response = AddProjectToCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to add project {} to collection {}: {:?}",
                    req.project_id, req.collection_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn remove_project_from_collection(
        &self,
        request: Request<RemoveProjectFromCollectionRequest>,
    ) -> Result<Response<RemoveProjectFromCollectionResponse>, Status> {
        debug!("RemoveProjectFromCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.remove_project_from_collection(&req.collection_id, &req.project_id) {
            Ok(()) => {
                debug!(
                    "Successfully removed project {} from collection {}",
                    req.project_id, req.collection_id
                );
                let response = RemoveProjectFromCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to remove project {} from collection {}: {:?}",
                    req.project_id, req.collection_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn reorder_collection(
        &self,
        request: Request<ReorderCollectionRequest>,
    ) -> Result<Response<ReorderCollectionResponse>, Status> {
        debug!("ReorderCollection request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Validate that all project IDs exist in the collection
        let collection = match db.get_collection_by_id(&req.collection_id) {
            Ok(Some((_, _, _, _, _, _, project_ids, _))) => project_ids,
            Ok(None) => {
                return Err(Status::new(Code::NotFound, "Collection not found"));
            }
            Err(e) => {
                error!("Failed to get collection {}: {:?}", req.collection_id, e);
                return Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ));
            }
        };

        // Check if all provided project IDs are in the collection
        let collection_set: std::collections::HashSet<_> = collection.iter().collect();
        let request_set: std::collections::HashSet<_> = req.project_ids.iter().collect();
        
        if collection_set != request_set {
            return Err(Status::new(
                Code::InvalidArgument,
                "Project IDs must match exactly with the collection's projects",
            ));
        }

        // Reorder the projects by updating their positions
        for (new_position, project_id) in req.project_ids.iter().enumerate() {
            match db.reorder_project_in_collection(&req.collection_id, project_id, new_position as i32) {
                Ok(()) => {
                    debug!(
                        "Successfully moved project {} to position {} in collection {}",
                        project_id, new_position, req.collection_id
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to move project {} to position {} in collection {}: {:?}",
                        project_id, new_position, req.collection_id, e
                    );
                    return Err(Status::new(
                        Code::Internal,
                        format!("Database error: {}", e),
                    ));
                }
            }
        }

        debug!(
            "Successfully reordered collection {} with {} projects",
            req.collection_id,
            req.project_ids.len()
        );
        let response = ReorderCollectionResponse { success: true };
        Ok(Response::new(response))
    }

    pub async fn get_collection_tasks(
        &self,
        request: Request<GetCollectionTasksRequest>,
    ) -> Result<Response<GetCollectionTasksResponse>, Status> {
        debug!("GetCollectionTasks request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.get_collection_tasks(&req.collection_id) {
            Ok(tasks_data) => {
                let mut tasks = Vec::new();
                let mut completed_count = 0;

                for (id, project_name, description, completed, created_at) in tasks_data {
                    if completed {
                        completed_count += 1;
                    }

                    tasks.push(Task {
                        id,
                        project_id: project_name, // Using project_name in project_id field to show which project the task belongs to
                        description,
                        completed,
                        created_at,
                    });
                }

                let total_tasks = tasks.len() as i32;
                let pending_tasks = total_tasks - completed_count;
                let completion_rate = if total_tasks > 0 {
                    completed_count as f64 / total_tasks as f64
                } else {
                    0.0
                };

                let response = GetCollectionTasksResponse {
                    tasks,
                    total_tasks,
                    completed_tasks: completed_count,
                    pending_tasks,
                    completion_rate,
                };

                debug!(
                    "Successfully retrieved {} tasks for collection {}",
                    total_tasks, req.collection_id
                );
                Ok(Response::new(response))
            }
            Err(e) => {
                error!(
                    "Failed to get tasks for collection {}: {:?}",
                    req.collection_id, e
                );
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn search_collections(
        &self,
        request: Request<SearchCollectionsRequest>,
    ) -> Result<Response<SearchCollectionsResponse>, Status> {
        debug!("SearchCollections request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.search_collections(&req.query, req.limit, req.offset) {
            Ok((collections_data, total_count)) => {
                let mut collections = Vec::new();

                for (id, name, description) in collections_data {
                    // Get the full collection details including project IDs
                    match db.get_collection_by_id(&id) {
                        Ok(Some((
                            _,
                            _,
                            _,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                        ))) => {
                            // Get collection statistics
                            let (total_duration_seconds, project_count) =
                                db.get_collection_statistics(&id).unwrap_or((None, 0));

                            collections.push(Collection {
                                id: id.clone(),
                                name: name.clone(),
                                description,
                                notes,
                                created_at,
                                modified_at,
                                project_ids,
                                cover_art_id,
                                total_duration_seconds,
                                project_count,
                            });
                        }
                        Ok(None) => {
                            // Collection was deleted between search and get_collection_by_id
                            debug!("Collection {} not found during detailed lookup", id);
                        }
                        Err(e) => {
                            error!("Failed to get collection details for {}: {:?}", id, e);
                            return Err(Status::new(
                                Code::Internal,
                                format!("Database error: {}", e),
                            ));
                        }
                    }
                }

                let response = SearchCollectionsResponse {
                    collections,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to search collections: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    // Batch Collection Operations
    pub async fn batch_add_to_collection(
        &self,
        request: Request<BatchAddToCollectionRequest>,
    ) -> Result<Response<BatchAddToCollectionResponse>, Status> {
        debug!("BatchAddToCollection request: {:?}", request);
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        match db.batch_add_projects_to_collection(&req.project_ids, &req.collection_id) {
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
                Ok(Response::new(BatchAddToCollectionResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                }))
            }
            Err(e) => Err(Status::internal(format!("Database error: {}", e))),
        }
    }

    pub async fn batch_remove_from_collection(
        &self,
        request: Request<BatchRemoveFromCollectionRequest>,
    ) -> Result<Response<BatchRemoveFromCollectionResponse>, Status> {
        debug!("BatchRemoveFromCollection request: {:?}", request);
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        match db.batch_remove_projects_from_collection(&req.project_ids, &req.collection_id) {
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
                Ok(Response::new(BatchRemoveFromCollectionResponse {
                    results: batch_results,
                    successful_count,
                    failed_count,
                }))
            }
            Err(e) => Err(Status::internal(format!("Database error: {}", e))),
        }
    }

    pub async fn batch_create_collection_from(
        &self,
        request: Request<BatchCreateCollectionFromRequest>,
    ) -> Result<Response<BatchCreateCollectionFromResponse>, Status> {
        debug!("BatchCreateCollectionFrom request: {:?}", request);
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        match db.batch_create_collection_from_projects(
            &req.collection_name,
            &req.project_ids,
            req.description.as_deref(),
            req.notes.as_deref(),
        ) {
            Ok((collection_id, results)) => {
                // Get the created collection details to return in response
                let collection = match db.get_collection_by_id(&collection_id) {
                    Ok(Some((
                        id,
                        name,
                        description,
                        notes,
                        created_at,
                        modified_at,
                        project_ids,
                        cover_art_id,
                    ))) => {
                        let (total_duration_seconds, project_count) =
                            db.get_collection_statistics(&id).unwrap_or((None, 0));
                        Some(Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                            total_duration_seconds,
                            project_count,
                        })
                    }
                    _ => None,
                };
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
                Ok(Response::new(BatchCreateCollectionFromResponse {
                    collection,
                    results: batch_results,
                    successful_count,
                    failed_count,
                }))
            }
            Err(e) => Err(Status::internal(format!("Database error: {}", e))),
        }
    }
}
