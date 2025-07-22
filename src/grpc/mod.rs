pub mod handlers;
pub mod server;

// Include the generated protobuf code for each service
pub mod common {
    tonic::include_proto!("studio_project_manager.common");
}

pub mod projects {
    tonic::include_proto!("studio_project_manager.projects");
}

pub mod collections {
    tonic::include_proto!("studio_project_manager.collections");
}

pub mod tasks {
    tonic::include_proto!("studio_project_manager.tasks");
}

pub mod search {
    tonic::include_proto!("studio_project_manager.search");
}

pub mod tags {
    tonic::include_proto!("studio_project_manager.tags");
}

pub mod media {
    tonic::include_proto!("studio_project_manager.media");
}

pub mod system {
    tonic::include_proto!("studio_project_manager.system");
}

pub mod plugins {
    tonic::include_proto!("studio_project_manager.plugins");
}

pub mod samples {
    tonic::include_proto!("studio_project_manager.samples");
}

pub mod scanning {
    tonic::include_proto!("studio_project_manager.scanning");
}

pub mod watcher {
    tonic::include_proto!("studio_project_manager.watcher");
}

pub use server::StudioProjectManagerServer;
