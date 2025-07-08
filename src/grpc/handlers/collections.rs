use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Code};
use log::{debug, error};

use crate::database::LiveSetDatabase;
use super::super::proto::*;

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

pub struct CollectionsHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl CollectionsHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }
    
    pub async fn get_collections(
        &self,
        _request: Request<GetCollectionsRequest>,
    ) -> Result<Response<GetCollectionsResponse>, Status> {
        debug!("GetCollections request");
        
        let mut db = self.db.lock().await;
        match db.list_collections() {
            Ok(collections_data) => {
                let mut collections = Vec::new();
                
                for (id, name, description) in collections_data {
                    // Get the full collection details including project IDs
                    match db.get_collection_by_id(&id) {
                        Ok(Some((_, _, _, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                            collections.push(Collection {
                                id: id.clone(),
                                name: name.clone(),
                                description,
                                notes,
                                created_at,
                                modified_at,
                                project_ids,
                                cover_art_id,
                            });
                        }
                        Ok(None) => {
                            // Collection was deleted between list_collections and get_collection_by_id
                            debug!("Collection {} not found during detailed lookup", id);
                        }
                        Err(e) => {
                            error!("Failed to get collection details for {}: {:?}", id, e);
                            return Err(Status::new(Code::Internal, format!("Database error: {}", e)));
                        }
                    }
                }
                
                let response = GetCollectionsResponse { collections };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get collections: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                        };
                        
                        let response = CreateCollectionResponse { collection: Some(collection) };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} was created but not found", collection_id);
                        Err(Status::new(Code::Internal, "Collection creation failed"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve created collection {}: {:?}", collection_id, e);
                        Err(Status::new(Code::Internal, format!("Database error: {}", e)))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create collection '{}': {:?}", req.name, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
                        };
                        
                        let response = UpdateCollectionResponse { collection: Some(collection) };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} not found after update", req.collection_id);
                        Err(Status::new(Code::NotFound, "Collection not found"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve updated collection {}: {:?}", req.collection_id, e);
                        Err(Status::new(Code::Internal, format!("Database error: {}", e)))
                    }
                }
            }
            Err(e) => {
                error!("Failed to update collection '{}': {:?}", req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                debug!("Successfully added project {} to collection {}", req.project_id, req.collection_id);
                let response = AddProjectToCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to add project {} to collection {}: {:?}", req.project_id, req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
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
                debug!("Successfully removed project {} from collection {}", req.project_id, req.collection_id);
                let response = RemoveProjectFromCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove project {} from collection {}: {:?}", req.project_id, req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }
} 