use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Code};
use log::{debug, error};

use crate::database::LiveSetDatabase;
use crate::grpc::proto::*;

// MOVE FROM server.rs:
// - get_project_tasks method (lines ~542-564)
//   Handles GetProjectTasksRequest
//   Calls db.get_project_tasks() and converts to proto Task format
//
// - create_task method (lines ~566-594)
//   Handles CreateTaskRequest
//   Calls db.add_task() and db.get_task() to return created task
//
// - update_task method (lines ~596-636)
//   Handles UpdateTaskRequest
//   Calls db.update_task_description() and db.complete_task() as needed
//   Returns updated task from db.get_task()
//
// - delete_task method (lines ~638-666)
//   Handles DeleteTaskRequest
//   Calls db.get_task() to check existence, then db.remove_task()

pub struct TasksHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl TasksHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_project_tasks(
        &self,
        request: Request<GetProjectTasksRequest>,
    ) -> Result<Response<GetProjectTasksResponse>, Status> {
        debug!("GetProjectTasks request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.get_project_tasks(&req.project_id) {
            Ok(task_data) => {
                let tasks = task_data.into_iter()
                    .map(|(id, description, completed, created_at)| Task {
                        id,
                        project_id: req.project_id.clone(),
                        description,
                        completed,
                        created_at,
                    })
                    .collect();
                
                let response = GetProjectTasksResponse { tasks };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get tasks for project {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn create_task(
        &self,
        request: Request<CreateTaskRequest>,
    ) -> Result<Response<CreateTaskResponse>, Status> {
        debug!("CreateTask request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.add_task(&req.project_id, &req.description) {
            Ok(task_id) => {
                // Get the created task to return full details
                match db.get_task(&task_id) {
                    Ok(Some((id, project_id, description, completed, created_at))) => {
                        let task = Task {
                            id,
                            project_id,
                            description,
                            completed,
                            created_at,
                        };
                        
                        let response = CreateTaskResponse { task: Some(task) };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Task was created but could not be retrieved: {}", task_id);
                        Err(Status::internal("Task created but could not be retrieved"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve created task {}: {:?}", task_id, e);
                        Err(Status::internal(format!("Database error: {}", e)))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create task for project {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn update_task(
        &self,
        request: Request<UpdateTaskRequest>,
    ) -> Result<Response<UpdateTaskResponse>, Status> {
        debug!("UpdateTask request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        // Update description if provided
        if let Some(description) = req.description {
            if let Err(e) = db.update_task_description(&req.task_id, &description) {
                error!("Failed to update task description for {}: {:?}", req.task_id, e);
                return Err(Status::new(Code::Internal, format!("Database error: {}", e)));
            }
        }
        
        // Update completion status if provided
        if let Some(completed) = req.completed {
            if let Err(e) = db.complete_task(&req.task_id, completed) {
                error!("Failed to update task completion status for {}: {:?}", req.task_id, e);
                return Err(Status::new(Code::Internal, format!("Database error: {}", e)));
            }
        }
        
        // Get the updated task to return
        match db.get_task(&req.task_id) {
            Ok(Some((id, project_id, description, completed, created_at))) => {
                let task = Task {
                    id,
                    project_id,
                    description,
                    completed,
                    created_at,
                };
                
                let response = UpdateTaskResponse { task: Some(task) };
                Ok(Response::new(response))
            }
            Ok(None) => {
                error!("Task not found: {}", req.task_id);
                Err(Status::not_found(format!("Task not found: {}", req.task_id)))
            }
            Err(e) => {
                error!("Failed to retrieve updated task {}: {:?}", req.task_id, e);
                Err(Status::internal(format!("Database error: {}", e)))
            }
        }
    }

    pub async fn delete_task(
        &self,
        request: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        debug!("DeleteTask request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        // Check if task exists first
        match db.get_task(&req.task_id) {
            Ok(Some(_)) => {
                // Task exists, proceed with deletion
                match db.remove_task(&req.task_id) {
                    Ok(()) => {
                        debug!("Successfully deleted task: {}", req.task_id);
                        let response = DeleteTaskResponse { success: true };
                        Ok(Response::new(response))
                    }
                    Err(e) => {
                        error!("Failed to delete task {}: {:?}", req.task_id, e);
                        Err(Status::new(Code::Internal, format!("Database error: {}", e)))
                    }
                }
            }
            Ok(None) => {
                error!("Task not found: {}", req.task_id);
                Err(Status::not_found(format!("Task not found: {}", req.task_id)))
            }
            Err(e) => {
                error!("Failed to check if task exists {}: {:?}", req.task_id, e);
                Err(Status::internal(format!("Database error: {}", e)))
            }
        }
    }
} 