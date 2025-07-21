pub mod handlers;
pub mod server;

// Include the generated protobuf code
pub mod proto {
    tonic::include_proto!("studio_project_manager");
}

pub use server::StudioProjectManagerServer;
