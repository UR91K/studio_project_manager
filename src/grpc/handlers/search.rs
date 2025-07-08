use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Code};
use log::{debug, error};

use crate::database::LiveSetDatabase;
use crate::database::search::SearchQuery;
use crate::LiveSet;
use super::super::proto::*;
use super::utils::convert_live_set_to_proto;

// MOVE FROM server.rs:
// - search method (lines ~169-207)
//   Handles SearchRequest with pagination
//   Calls db.search() and converts results to proto
//   Uses SearchQuery::parse() for query parsing

pub struct SearchHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl SearchHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        debug!("Search request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        let _search_query = SearchQuery::parse(&req.query);
        
        match db.search(&req.query) {
            Ok(search_results) => {
                let total_count = search_results.len() as i32;
                let results_iter = search_results.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let mut proto_projects = Vec::new();
                
                let projects_to_convert: Vec<LiveSet> = if let Some(limit) = req.limit {
                    results_iter.take(limit as usize).collect()
                } else {
                    results_iter.collect()
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
                
                let response = SearchResponse {
                    projects: proto_projects,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Search failed for query '{}': {:?}", req.query, e);
                Err(Status::new(Code::Internal, format!("Search error: {}", e)))
            }
        }
    }
} 