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
    
    // Initialize system monitor
    initialize_system_monitor();
    
    // Initialize database (in-memory for testing)
    info!("📊 Initializing in-memory database for testing...");
    let database = match DatabaseManager::new(":memory:").await {
        Ok(db) => {
            info!("📊 Database ready");
            Arc::new(db)
        }
        Err(e) => {
            error!("❌ Failed to initialize database: {}", e);
            return Err(e);
        }
    };
    
    // Start embedded Redis server
    info!("🔴 Starting embedded Redis server...");
    let mut redis_server = EmbeddedRedisServer::new(Some(6379));
    if let Err(e) = redis_server.start().await {
        warn!("⚠️ Failed to start embedded Redis server: {}", e);
    } else {
        // Wait for Redis to be ready
        if redis_server.wait_for_ready(5000).await {
            info!("🔴 Embedded Redis server is ready");
        } else {
            warn!("⚠️ Embedded Redis server did not become ready in time");
        }
    }
    
    // Keep the Redis server alive by moving it into a static or long-lived context
    // We'll leak it intentionally since it should live for the entire program duration
    let _redis_server_handle = Box::leak(Box::new(redis_server));

    // Initialize Redis cache (now connects to our embedded server)
    info!("🔴 Initializing Redis cache...");
    let redis_cache = Arc::new(tokio::sync::Mutex::new(RedisCache::new()));
    
    // Initialize RenScript cache
    info!("📜 Initializing RenScript cache...");
    let renscript_cache = Arc::new(RenScriptCache::new(None)); // For now, skip Redis integration
    
    // Initialize the RenScript cache with directory scanning
    let renscripts_path = base_path.join("renscripts");
    if let Err(e) = renscript_cache.initialize(&renscripts_path).await {
        warn!("⚠️ Failed to initialize RenScript cache: {}", e);
    }
    
    // Set state in handlers module
    set_startup_time(startup_time);
    set_database(database);
    set_redis_cache(redis_cache);
    set_renscript_cache(renscript_cache);
    
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