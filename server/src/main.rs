use std::env;
use std::process::Command;
use actix_web::{web, App, HttpServer, HttpRequest, HttpResponse, Result as ActixResult, middleware::Logger};
use actix_cors::Cors;
use tracing::{info, warn};

mod modules;
use modules::*;
use modules::file_watcher;

/// Kill any process running on the specified port
async fn kill_port_process(port: u16) {
    info!("🧹 Checking for existing processes on port {}", port);
    
    #[cfg(windows)]
    {
        // Windows: Use netstat and taskkill
        let netstat_output = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output();
            
        if let Ok(output) = netstat_output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            for line in output_str.lines() {
                if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                    // Extract PID (last column)
                    if let Some(pid_str) = line.split_whitespace().last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            info!("🔍 Found process {} using port {}, terminating...", pid, port);
                            
                            let kill_result = Command::new("taskkill")
                                .args(["/F", "/PID", &pid.to_string()])
                                .output();
                                
                            match kill_result {
                                Ok(_) => info!("✅ Successfully terminated process {}", pid),
                                Err(e) => warn!("⚠️ Failed to terminate process {}: {}", pid, e),
                            }
                        }
                    }
                }
            }
        }
    }
    
    #[cfg(unix)]
    {
        // Unix: Use lsof and kill
        let lsof_output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output();
            
        if let Ok(output) = lsof_output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            for line in output_str.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    info!("🔍 Found process {} using port {}, terminating...", pid, port);
                    
                    let kill_result = Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                        
                    match kill_result {
                        Ok(_) => info!("✅ Successfully terminated process {}", pid),
                        Err(e) => warn!("⚠️ Failed to terminate process {}: {}", pid, e),
                    }
                }
            }
        }
    }
    
    // Give processes time to clean up
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    info!("🚀 Starting Renzora Server (High-Performance WebSocket)");
    
    // Load configuration
    let config = Config::load();
    let port_str = config.get_port();
    let port: u16 = port_str.parse().unwrap_or(3002);
    
    // Kill any existing process on our port
    kill_port_process(port).await;
    let host = config.get_host();
    let workers = config.get_workers();
    let base_path = config.get_base_path();
    let projects_path = config.get_projects_path();
    
    // Initialize configuration manager for runtime updates
    init_config_manager(config);
    
    info!("🌐 Server will run on: http://{}:{}", host, port);
    info!("🔧 Workers: {}", workers);
    info!("📂 Base path: {}", base_path.display());
    info!("📂 Projects path: {}", projects_path.display());
    
    // Ensure projects directory exists
    if !projects_path.exists() {
        std::fs::create_dir_all(&projects_path)?;
        info!("📁 Created projects directory");
    }
    
    // Initialize server state
    let app_state = AppState::new().await.expect("Failed to initialize server state");
    info!("✅ Server state initialized");
    
    // Initialize file watcher
    let _file_watcher = file_watcher::initialize_file_watcher(projects_path.clone(), app_state.clone()).await
        .expect("Failed to initialize file watcher");
    info!("👀 File watcher initialized");
    
    info!("🎯 Renzora Server ready to accept connections");
    
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_origin("http://127.0.0.1:3000")
                    .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
                    .allowed_headers(vec!["Content-Type", "Authorization"])
                    .max_age(3600)
            )
            .wrap(Logger::default())
            .route("/ws", web::get().to(websocket_handler))
            .route("/health", web::get().to(health_check))
    })
    .bind(format!("{}:{}", host, port))?
    .workers(workers) // Use configured worker count
    .run()
    .await
}

async fn websocket_handler(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let (response, session, stream) = actix_ws::handle(&req, body)?;
    
    info!("🔌 New WebSocket connection established");
    
    // Spawn WebSocket handler
    actix_web::rt::spawn(handle_websocket_connection(session, stream, state.get_ref().clone()));
    
    Ok(response)
}

async fn handle_websocket_connection(
    session: actix_ws::Session,
    stream: actix_ws::MessageStream,
    state: AppState,
) {
    // This will be implemented in the websocket module
    websocket::handle_connection(session, stream, state).await;
}

async fn health_check() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "server": "Renzora Server",
        "version": env!("CARGO_PKG_VERSION")
    })))
}