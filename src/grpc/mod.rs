pub mod handlers;
pub mod server;

// Include the generated protobuf code for each service
pub mod common {
    tonic::include_proto!("seula.common");
}

pub mod projects {
    tonic::include_proto!("seula.projects");
}

pub mod collections {
    tonic::include_proto!("seula.collections");
}

pub mod tasks {
    tonic::include_proto!("seula.tasks");
}

pub mod search {
    tonic::include_proto!("seula.search");
}

pub mod tags {
    tonic::include_proto!("seula.tags");
}

pub mod media {
    tonic::include_proto!("seula.media");
}

pub mod system {
    tonic::include_proto!("seula.system");
}

pub mod plugins {
    tonic::include_proto!("seula.plugins");
}

pub mod samples {
    tonic::include_proto!("seula.samples");
}

pub mod scanning {
    tonic::include_proto!("seula.scanning");
}

pub mod watcher {
    tonic::include_proto!("seula.watcher");
}

pub mod config {
    tonic::include_proto!("seula.config");
}

pub use server::StudioProjectManagerServer;
