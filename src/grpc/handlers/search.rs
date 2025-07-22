use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use super::super::search::*;
use super::utils::convert_live_set_to_proto;
use crate::database::search::SearchQuery;
use crate::database::LiveSetDatabase;

#[derive(Clone)]
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

        let search_query = SearchQuery::parse(&req.query);

        match db.search_fts(&search_query) {
            Ok(search_results) => {
                let total_count = search_results.len() as i32;
                let results_iter = search_results
                    .into_iter()
                    .skip(req.offset.unwrap_or(0) as usize);
                let mut proto_projects = Vec::new();

                let results_to_convert: Vec<_> = if let Some(limit) = req.limit {
                    results_iter.take(limit as usize).collect()
                } else {
                    results_iter.collect()
                };

                for search_result in results_to_convert {
                    match convert_live_set_to_proto(search_result.project, &mut *db) {
                        Ok(proto_project) => {
                            proto_projects.push(proto_project);
                        }
                        Err(e) => {
                            error!("Failed to convert project to proto: {}", e);
                            return Err(Status::internal(format!(
                                "Failed to convert project: {}",
                                e
                            )));
                        }
                    }
                }

                Ok(Response::new(SearchResponse {
                    projects: proto_projects,
                    total_count,
                }))
            }
            Err(e) => {
                error!("Search failed: {}", e);
                Err(Status::internal(format!("Search failed: {}", e)))
            }
        }
    }
}
