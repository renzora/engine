use std::fs;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

mod modules;
use modules::*;
use modules::update_manager::check_for_updates;

// Shared application state
#[derive(Clone, Debug)]
pub struct AppState {
    pub startup_time: u64, // Unix timestamp when bridge started
}

// Global state instance
use std::sync::OnceLock;
static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

pub fn get_app_state() -> Arc<AppState> {
    APP_STATE.get().expect("App state not initialized").clone()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize application state with startup time
    let startup_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    
    let state = Arc::new(AppState { startup_time });
    APP_STATE.set(state.clone()).expect("Failed to set app state");
    
    // Also set startup time in handlers module
    set_startup_time(startup_time);
    
    let port = env::var("BRIDGE_PORT").unwrap_or_else(|_| "3001".to_string());
    let base_path = get_base_path();
    let projects_path = get_projects_path();
    
    println!("🌉 Starting battle-tested Bridge API server with hyper...");
    println!("📂 Base path: {}", base_path.display());
    println!("📂 Projects path: {}", projects_path.display());
    println!("🔌 Running on: http://localhost:{}", port);
    println!("⏰ Bridge started at: {} (Unix timestamp)", startup_time);
    
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
    
    // Check for updates on startup
    println!("🔄 Checking for updates...");
    tokio::spawn(async {
        match check_for_updates().await {
            Ok(result) => {
                if result.update_available {
                    println!("🎉 Update available!");
                    match result.channel {
                        modules::update_manager::Channel::Stable => {
                            if let Some(version) = result.latest_stable_version {
                                println!("   Latest stable version: {}", version);
                            }
                        }
                        modules::update_manager::Channel::Dev => {
                            if let Some(version) = result.latest_dev_version {
                                println!("   Latest dev version: {}", version);
                            }
                        }
                    }
                    if let Some(url) = result.download_url {
                        println!("   Download: {}", url);
                    }
                } else {
                    println!("✅ You're running the latest version!");
                }
            }
            Err(e) => {
                println!("⚠️  Failed to check for updates: {}", e);
            }
        }
    });

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