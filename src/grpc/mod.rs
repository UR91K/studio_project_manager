pub mod server;
pub mod handlers;

// Include the generated protobuf code
pub mod proto {
    tonic::include_proto!("studio_project_manager");
}

pub use server::StudioProjectManagerServer; 