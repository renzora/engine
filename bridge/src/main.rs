use std::fs;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use log::{info, warn, error};

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
    // Initialize logger with timestamp and colors
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .init();
    
    info!("🚀 Initializing Bridge API server...");
    
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
    
    info!("🌉 Starting battle-tested Bridge API server with hyper...");
    info!("📂 Base path: {}", base_path.display());
    info!("📂 Projects path: {}", projects_path.display());
    info!("🔌 Running on: http://localhost:{}", port);
    info!("⏰ Bridge started at: {} (Unix timestamp)", startup_time);
    
    if !projects_path.exists() {
        fs::create_dir_all(&projects_path)?;
        info!("📁 Created projects directory");
    }
    
    // Initialize file watcher
    initialize_file_watcher(projects_path.clone())?;
    
    // File watching now uses SSE streaming endpoint instead of separate WebSocket server
    
    // Check for updates on startup
    info!("🔄 Checking for updates...");
    tokio::spawn(async {
        match check_for_updates().await {
            Ok(result) => {
                if result.update_available {
                    info!("🎉 Update available!");
                    match result.channel {
                        modules::update_manager::Channel::Stable => {
                            if let Some(version) = result.latest_stable_version {
                                info!("   Latest stable version: {}", version);
                            }
                        }
                        modules::update_manager::Channel::Dev => {
                            if let Some(version) = result.latest_dev_version {
                                info!("   Latest dev version: {}", version);
                            }
                        }
                    }
                    if let Some(url) = result.download_url {
                        info!("   Download: {}", url);
                    }
                } else {
                    info!("✅ You're running the latest version!");
                }
            }
            Err(e) => {
                warn!("⚠️  Failed to check for updates: {}", e);
            }
        }
    });

    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("🎯 Server ready to accept connections");

    loop {
        let (tcp, client_addr) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        
        info!("🔗 New connection from: {}", client_addr);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_http_request))
                .await
            {
                error!("❌ Error serving connection from {}: {:?}", client_addr, err);
            }
        });
    }
}