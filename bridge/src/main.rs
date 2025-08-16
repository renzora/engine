use std::fs;
use std::env;
use std::net::SocketAddr;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

mod modules;
use modules::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("BRIDGE_PORT").unwrap_or_else(|_| "3001".to_string());
    let base_path = get_base_path();
    let projects_path = get_projects_path();
    
    println!("🌉 Starting battle-tested Bridge API server with hyper...");
    println!("📂 Base path: {}", base_path.display());
    println!("📂 Projects path: {}", projects_path.display());
    println!("🔌 Running on: http://localhost:{}", port);
    
    if !projects_path.exists() {
        fs::create_dir_all(&projects_path)?;
        println!("📁 Created projects directory");
    }
    
    // Check if default project exists, create if not
    let default_project_path = projects_path.join("test-project");
    if !default_project_path.exists() {
        println!("📁 Creating default project structure...");
        fs::create_dir_all(&default_project_path)?;
        
        // Create standard asset directories
        let asset_dirs = ["models", "images", "audio", "video", "scripts", "textures", "materials"];
        for dir in &asset_dirs {
            let dir_path = default_project_path.join(dir);
            fs::create_dir_all(&dir_path)?;
            println!("📁 Created directory: {}", dir);
        }
        
        println!("✅ Default project 'test-project' created with asset directories");
    }
    
    // Initialize file watcher
    initialize_file_watcher(projects_path.clone())?;

    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_http_request))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}