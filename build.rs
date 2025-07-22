use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _proto_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("proto");
    
    println!("cargo:rerun-if-changed=proto/");
    
    // Compile common.proto first
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(&["proto/common.proto"], &["proto/"])?;

    // Compile each service separately
    let services = [
        "projects",
        "collections", 
        "tasks",
        "search",
        "tags",
        "media",
        "system",
        "plugins",
        "samples",
        "scanning",
        "watcher"
    ];

    for service in &services {
        let proto_file = format!("proto/services/{}.proto", service);
        println!("cargo:rerun-if-changed={}", proto_file);
        
        tonic_build::configure()
            .build_server(true)
            .build_client(false)
            .compile(&[&proto_file], &["proto/"])?;
    }

    Ok(())
}