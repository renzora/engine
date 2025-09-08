use hyper::{Request, Response, Method, StatusCode};
use hyper::header::{CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_HEADERS};
use http_body_util::{BodyExt, Full, StreamBody, combinators::BoxBody};
use hyper::body::Frame;
use bytes::Bytes;
use async_stream::stream;
use std::convert::Infallible;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use log::{info, warn, error, debug};
use percent_encoding::percent_decode_str;
use base64::{Engine as _, engine::general_purpose};
use crate::types::{ApiResponse, WriteFileRequest, WriteBinaryFileRequest, CreateProjectRequest};
use crate::project_manager::{list_projects, list_directory_contents, create_project, load_scene_with_assets};
use crate::file_sync::{read_file_content, write_file_content, delete_file_or_directory, get_file_content_type, read_binary_file, write_binary_file_content};
use crate::thumbnail_generator::{get_or_generate_thumbnail, ThumbnailRequest, batch_generate_thumbnails, generate_model_thumbnail};
use crate::update_manager::{Channel, check_for_updates, set_update_channel, get_current_config, get_last_update_check};
use crate::file_watcher::{get_file_change_receiver, set_current_project};
use crate::system_monitor::get_system_stats;
use crate::model_processor::{process_model_import, extract_model_settings, SceneAnalysis};
use crate::model_converter::{convert_model_to_glb_and_extract, ImportMode, CompressionSettings};
use crate::project_manager::get_projects_path;
use crate::renscript_compiler::compile_renscript;
use std::fs;
use crate::database::DatabaseManager;
use crate::redis_cache::RedisCache;
use crate::renscript_cache::RenScriptCache;

// Static variables for shared state
static STARTUP_TIME: OnceLock<u64> = OnceLock::new();
static DATABASE: OnceLock<Arc<DatabaseManager>> = OnceLock::new();
static REDIS_CACHE: OnceLock<Arc<tokio::sync::Mutex<RedisCache>>> = OnceLock::new();
static RENSCRIPT_CACHE: OnceLock<Arc<RenScriptCache>> = OnceLock::new();

pub fn set_startup_time(timestamp: u64) {
    STARTUP_TIME.set(timestamp).ok();
}

pub fn set_database(database: Arc<DatabaseManager>) {
    DATABASE.set(database).ok();
}

pub fn set_redis_cache(redis_cache: Arc<tokio::sync::Mutex<RedisCache>>) {
    REDIS_CACHE.set(redis_cache).ok();
}

pub fn set_renscript_cache(renscript_cache: Arc<RenScriptCache>) {
    RENSCRIPT_CACHE.set(renscript_cache).ok();
}

pub async fn handle_http_request(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    let start_time = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let user_agent = req.headers().get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    
    info!("📥 {} {} {} - User-Agent: {}", method, path, 
          if query.is_empty() { "".to_string() } else { format!("?{}", query) }, user_agent);
    
    // Handle CORS preflight
    if method == Method::OPTIONS {
        let duration = start_time.elapsed();
        info!("⚡ OPTIONS {} - 200 OK - {}ms", path, duration.as_millis());
        return Ok(cors_response(StatusCode::OK, ""));
    }
    
    // Special handling for SSE streaming endpoint - return streaming response
    if method == Method::GET && path == "/file-changes/stream" {
        let duration = start_time.elapsed();
        info!("🔄 SSE {} - streaming connection established - {}ms", path, duration.as_millis());
        return Ok(create_sse_stream_response());
    }
    
    // Read body if this is a POST request
    let body = if method == Method::POST {
        match read_request_body(req).await {
            Ok(body) => {
                let body_size = body.len();
                info!("📄 Request body size: {} bytes", body_size);
                if body_size > 1024 {
                    debug!("📄 Large request body ({}KB)", body_size / 1024);
                }
                Some(body)
            },
            Err(e) => {
                error!("❌ Failed to read request body: {}", e);
                return Ok(error_response(StatusCode::BAD_REQUEST, "Failed to read request body"));
            }
        }
    } else {
        None
    };
    
    let response = match (&method, path.as_str()) {
        (&Method::GET, "/projects") => handle_get_projects(),
        (&Method::POST, "/projects") => {
            match &body {
                Some(body_content) => handle_create_project(body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, path) if path.starts_with("/list/") => {
            let dir_path = &path[6..];
            let decoded_path = match decode_url_path(dir_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_list_directory(&decoded_path)
        }
        (&Method::GET, path) if path.starts_with("/read/") => {
            let file_path = &path[6..];
            let decoded_path = match decode_url_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_read_file(&decoded_path)
        }
        (&Method::POST, path) if path.starts_with("/write/") => {
            let file_path = &path[7..];
            let decoded_path = match decode_url_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            match &body {
                Some(body_content) => handle_write_file(&decoded_path, body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, path) if path.starts_with("/write-binary/") => {
            let file_path = &path[14..];
            let decoded_path = match decode_url_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            match &body {
                Some(body_content) => handle_write_binary_file(&decoded_path, body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::DELETE, path) if path.starts_with("/delete/") => {
            let file_path = &path[8..];
            handle_delete_file(file_path)
        }
        (&Method::GET, path) if path.starts_with("/script/") => {
            let script_name = &path[8..];
            let decoded_name = match decode_url_path(script_name) {
                Ok(name) => name,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            return Ok(handle_compile_script(&decoded_name).await);
        }
        (&Method::GET, "/scripts/search") => {
            return Ok(handle_search_scripts(&query).await);
        }
        (&Method::GET, "/scripts") => {
            return Ok(handle_list_scripts().await);
        }
        (&Method::POST, "/scripts/cache/clear") => {
            return Ok(handle_clear_script_cache().await);
        }
        (&Method::GET, "/scripts/cache/stats") => {
            return Ok(handle_cache_stats().await);
        }
        (&Method::GET, "/renscripts/search") => {
            return Ok(handle_search_renscripts(&query).await);
        }
        (&Method::GET, "/renscripts") => {
            return Ok(handle_list_renscripts().await);
        }
        (&Method::POST, "/renscripts/cache/refresh") => {
            return Ok(handle_refresh_renscript_cache().await);
        }
        (&Method::POST, "/database/query") => {
            match &body {
                Some(body_content) => return Ok(handle_database_query(body_content).await),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, path) if path.starts_with("/file/") => {
            let file_path = &path[6..];
            let decoded_path = match decode_url_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            return Ok(handle_serve_asset(&decoded_path));
        }
        (&Method::POST, "/start-watcher") => handle_start_watcher(),
        (&Method::GET, "/file-changes") => handle_get_file_changes(),
        (&Method::POST, "/clear-changes") => handle_clear_file_changes(),
        (&Method::POST, "/set-current-project") => {
            match &body {
                Some(body_content) => handle_set_current_project(body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, "/thumbnail") => {
            match &body {
                Some(body_content) => handle_generate_thumbnail(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, "/thumbnails/batch") => {
            match &body {
                Some(body_content) => handle_batch_generate_thumbnails(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, "/health") => handle_health_check(),
        (&Method::GET, "/startup-time") => handle_get_startup_time(),
        (&Method::GET, "/system/stats") => handle_get_system_stats(),
        (&Method::POST, "/restart") => handle_restart_bridge(),
        (&Method::POST, "/clear-cache") => handle_clear_cache(),
        (&Method::GET, "/update/check") => {
            handle_check_updates().await
        },
        (&Method::POST, "/update/channel") => {
            match &body {
                Some(body_content) => handle_set_update_channel(body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, "/update/config") => handle_get_update_config(),
        (&Method::POST, "/update/apply") => {
            error_response(StatusCode::NOT_IMPLEMENTED, "Async endpoint - use web interface")
        },
        (&Method::POST, "/update/download") => {
            match &body {
                Some(_body_content) => {
                    error_response(StatusCode::NOT_IMPLEMENTED, "Async endpoint - use web interface")
                },
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, "/process-model") => {
            match &body {
                Some(body_content) => handle_process_model(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, "/update-model-summary") => {
            match &body {
                Some(body_content) => handle_update_model_summary(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, "/convert-to-glb") => {
            match &body {
                Some(body_content) => handle_convert_to_glb(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, path) if path.starts_with("/scene-bundle/") => {
            let path_parts: Vec<&str> = path[14..].split('/').collect();
            if path_parts.len() != 2 {
                error_response(StatusCode::BAD_REQUEST, "Expected format: /scene-bundle/PROJECT_NAME/SCENE_NAME")
            } else {
                let project_name = match decode_url_path(path_parts[0]) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                let scene_name = match decode_url_path(path_parts[1]) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                handle_get_scene_bundle(&project_name, &scene_name)
            }
        }
        _ => {
            warn!("❓ Unknown route: {} {}", method, path);
            error_response(StatusCode::NOT_FOUND, "Not Found")
        },
    };
    
    let duration = start_time.elapsed();
    let status = response.status();
    
    if status.is_success() {
        info!("✅ {} {} - {} - {}ms", method, path, status, duration.as_millis());
    } else if status.is_client_error() {
        warn!("⚠️  {} {} - {} - {}ms", method, path, status, duration.as_millis());
    } else {
        error!("❌ {} {} - {} - {}ms", method, path, status, duration.as_millis());
    }
    
    Ok(response)
}

async fn read_request_body(req: Request<hyper::body::Incoming>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = req.into_body();
    let body_bytes = body.collect().await?.to_bytes();
    Ok(String::from_utf8(body_bytes.to_vec())?)
}

fn cors_response(status: StatusCode, body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, DELETE, OPTIONS")
        .header(ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .header(CONTENT_TYPE, "application/json")
        .body(BoxBody::new(Full::new(Bytes::from(body.to_string()))))
        .unwrap()
}

fn json_response<T: serde::Serialize>(data: &T) -> Response<BoxBody<Bytes, Infallible>> {
    let json = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    cors_response(StatusCode::OK, &json)
}

fn error_response(status: StatusCode, message: &str) -> Response<BoxBody<Bytes, Infallible>> {
    let error_response = ApiResponse {
        success: false,
        content: None,
        error: Some(message.to_string()),
    };
    let json = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
    cors_response(status, &json)
}

fn handle_get_projects() -> Response<BoxBody<Bytes, Infallible>> {
    match list_projects() {
        Ok(projects) => json_response(&projects),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_list_directory(dir_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match list_directory_contents(dir_path) {
        Ok(files) => json_response(&files),
        Err(e) => error_response(StatusCode::FORBIDDEN, &e),
    }
}

fn handle_read_file(file_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match read_file_content(file_path) {
        Ok(content) => {
            let response = ApiResponse {
                success: true,
                content: Some(content),
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_get_scene_bundle(project_name: &str, scene_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match load_scene_with_assets(project_name, scene_name) {
        Ok(bundle) => json_response(&bundle),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_write_file(file_path: &str, body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    let write_req: WriteFileRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };

    match write_file_content(file_path, &write_req) {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                content: None,
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_write_binary_file(file_path: &str, body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    let write_req: WriteBinaryFileRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };

    match write_binary_file_content(file_path, &write_req) {
        Ok(_) => {
            // Check if this is a GLB file and generate thumbnail automatically
            if file_path.to_lowercase().ends_with(".glb") {
                // Parse project name from file path (format: "project_name/assets/...")
                let path_parts: Vec<&str> = file_path.split('/').collect();
                if path_parts.len() >= 2 {
                    let project_name = path_parts[0];
                    println!("📸 Auto-generating thumbnail for uploaded GLB: {}", file_path);
                    
                    // Generate thumbnails in background (don't block the upload response)
                    let project_name = project_name.to_string();
                    let file_path = file_path.to_string();
                    tokio::spawn(async move {
                        let sizes = [128, 256, 512];
                        for &size in &sizes {
                            match generate_model_thumbnail(&project_name, &file_path, size).await {
                                Ok(thumbnail_file) => {
                                    println!("✅ Generated {}px thumbnail: {}", size, thumbnail_file);
                                }
                                Err(e) => {
                                    println!("❌ Failed to generate {}px thumbnail for {}: {}", size, file_path, e);
                                }
                            }
                        }
                    });
                }
            }
            
            let response = ApiResponse {
                success: true,
                content: None,
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn decode_url_path(encoded_path: &str) -> Result<String, String> {
    match percent_decode_str(encoded_path).decode_utf8() {
        Ok(decoded) => Ok(decoded.to_string()),
        Err(e) => {
            error!("Failed to decode URL path '{}': {}", encoded_path, e);
            Err(format!("Invalid URL encoding: {}", e))
        }
    }
}

fn handle_delete_file(file_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
    let decoded_path = match decode_url_path(file_path) {
        Ok(path) => path,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e),
    };
    
    match delete_file_or_directory(&decoded_path) {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                content: None,
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_serve_asset(file_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match read_binary_file(file_path) {
        Ok(contents) => {
            let full_path = std::path::Path::new(file_path);
            let content_type = get_file_content_type(full_path);
            
            Response::builder()
                .status(StatusCode::OK)
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(CONTENT_TYPE, content_type)
                .body(BoxBody::new(Full::new(Bytes::from(contents))))
                .unwrap()
        }
        Err(e) => {
            let status = if e == "File not found" {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error_response(status, &e)
        }
    }
}

fn handle_start_watcher() -> Response<BoxBody<Bytes, Infallible>> {
    let response = ApiResponse {
        success: true,
        content: Some("File watcher is running".to_string()),
        error: None,
    };
    json_response(&response)
}

fn handle_get_file_changes() -> Response<BoxBody<Bytes, Infallible>> {
    json_response(&Vec::<String>::new())
}

fn handle_clear_file_changes() -> Response<BoxBody<Bytes, Infallible>> {
    let response = ApiResponse {
        success: true,
        content: None,
        error: None,
    };
    json_response(&response)
}

fn handle_set_current_project(body_content: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct SetProjectRequest {
        project_name: Option<String>,
    }
    
    match serde_json::from_str::<SetProjectRequest>(body_content) {
        Ok(request) => {
            set_current_project(request.project_name.clone());
            let response = ApiResponse {
                success: true,
                content: request.project_name.map(|n| format!("Now watching project: {}", n))
                    .or(Some("Now watching all projects".to_string())),
                error: None,
            };
            json_response(&response)
        }
        Err(e) => {
            error_response(StatusCode::BAD_REQUEST, &format!("Invalid request format: {}", e))
        }
    }
}

async fn handle_generate_thumbnail(body_content: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match serde_json::from_str::<ThumbnailRequest>(body_content) {
        Ok(request) => {
            let response = get_or_generate_thumbnail(request).await;
            json_response(&response)
        }
        Err(e) => {
            error_response(StatusCode::BAD_REQUEST, &format!("Invalid request format: {}", e))
        }
    }
}

#[derive(serde::Deserialize)]
struct BatchThumbnailRequest {
    project_name: String,
}

async fn handle_batch_generate_thumbnails(body_content: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match serde_json::from_str::<BatchThumbnailRequest>(body_content) {
        Ok(request) => {
            match batch_generate_thumbnails(&request.project_name).await {
                Ok(thumbnails) => {
                    let response = serde_json::json!({
                        "success": true,
                        "generated_count": thumbnails.len(),
                        "thumbnails": thumbnails
                    });
                    json_response(&response)
                }
                Err(e) => {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Batch generation failed: {}", e))
                }
            }
        }
        Err(e) => {
            error_response(StatusCode::BAD_REQUEST, &format!("Invalid request format: {}", e))
        }
    }
}

fn handle_create_project(body_content: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match serde_json::from_str::<CreateProjectRequest>(body_content) {
        Ok(request) => {
            match create_project(&request.name, &request.template.unwrap_or_else(|| "basic".to_string()), request.settings.as_ref()) {
                Ok(project) => json_response(&project),
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
            }
        }
        Err(e) => {
            error_response(StatusCode::BAD_REQUEST, &format!("Invalid request format: {}", e))
        }
    }
}

fn create_sse_stream_response() -> Response<BoxBody<Bytes, Infallible>> {
    let stream = stream! {
        // Send initial connection message
        let init_msg = "data: {\"type\":\"connected\",\"message\":\"File change stream connected\"}\n\n";
        yield Ok(Frame::data(Bytes::from(init_msg)));
        
        // Get file change receiver
        if let Some(mut receiver) = get_file_change_receiver() {
            info!("📡 SSE: Streaming connection established, listening for file changes");
            
            // Set up a heartbeat to keep connection alive
            let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                tokio::select! {
                    // Handle file change events
                    file_change = receiver.recv() => {
                        match file_change {
                            Ok(message) => {
                                info!("📡 SSE: Broadcasting file change: {}", message);
                                let sse_msg = format!("data: {{\"type\":\"file_change\",\"message\":\"{}\"}}\n\n", 
                                    message.replace("\"", "\\\""));
                                yield Ok(Frame::data(Bytes::from(sse_msg)));
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                                warn!("📡 SSE: Lagged {} messages", count);
                                let lag_msg = format!("data: {{\"type\":\"lag\",\"count\":{}}}\n\n", count);
                                yield Ok(Frame::data(Bytes::from(lag_msg)));
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                info!("📡 SSE: File change broadcaster closed");
                                break;
                            }
                        }
                    }
                    
                    // Send heartbeat to keep connection alive
                    _ = heartbeat_interval.tick() => {
                        let heartbeat_msg = "data: {\"type\":\"heartbeat\"}\n\n";
                        yield Ok(Frame::data(Bytes::from(heartbeat_msg)));
                    }
                }
            }
        } else {
            warn!("📡 SSE: No file change receiver available");
            let error_msg = "data: {\"type\":\"error\",\"message\":\"File watcher not available\"}\n\n";
            yield Ok(Frame::data(Bytes::from(error_msg)));
        }
    };
    
    Response::builder()
        .status(StatusCode::OK)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, DELETE, OPTIONS")
        .header(ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .header(CONTENT_TYPE, "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(BoxBody::new(StreamBody::new(stream)))
        .unwrap()
}


fn handle_health_check() -> Response<BoxBody<Bytes, Infallible>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    // Get basic system info
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let health_data = serde_json::json!({
        "status": "healthy",
        "uptime": uptime % 86400, // Reset daily for demo
        "cache_size": 1024 * 1024, // Mock cache size
        "thumbnail_count": 5, // Mock thumbnail count
        "watched_files": 12, // Mock watched files count
        "timestamp": uptime
    });
    
    json_response(&health_data)
}

fn handle_get_startup_time() -> Response<BoxBody<Bytes, Infallible>> {
    let startup_time = STARTUP_TIME.get().copied().unwrap_or_else(|| {
        // Fallback: return current time if not set
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });
    
    let startup_data = serde_json::json!({
        "startup_time": startup_time,
        "startup_time_ms": startup_time * 1000 // Also provide milliseconds for JavaScript
    });
    
    json_response(&startup_data)
}

fn handle_get_system_stats() -> Response<BoxBody<Bytes, Infallible>> {
    let stats = get_system_stats();
    json_response(&stats)
}


fn handle_restart_bridge() -> Response<BoxBody<Bytes, Infallible>> {
    // In a real implementation, this would restart the server
    // For now, just return success
    let response = ApiResponse {
        success: true,
        content: Some("Bridge restart initiated".to_string()),
        error: None,
    };
    json_response(&response)
}

fn handle_clear_cache() -> Response<BoxBody<Bytes, Infallible>> {
    // In a real implementation, this would clear caches
    // For now, just return success
    let response = ApiResponse {
        success: true,
        content: Some("Cache cleared successfully".to_string()),
        error: None,
    };
    json_response(&response)
}


fn handle_set_update_channel(body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct ChannelRequest {
        channel: String,
    }
    
    let channel_req: ChannelRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON"),
    };
    
    let channel = match channel_req.channel.as_str() {
        "stable" => Channel::Stable,
        "dev" => Channel::Dev,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid channel. Must be 'stable' or 'dev'"),
    };
    
    // Update the channel in the static config
    match set_update_channel(channel.clone()) {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                content: Some(format!("Update channel set to {}", channel)),
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e)
    }
}

fn handle_get_update_config() -> Response<BoxBody<Bytes, Infallible>> {
    let config = get_current_config();
    json_response(&config)
}

async fn handle_check_updates() -> Response<BoxBody<Bytes, Infallible>> {
    // First check if we have a cached result from startup
    if let Some(cached_result) = get_last_update_check() {
        // If we have a cached result, return it immediately
        // and trigger a new check in the background
        tokio::spawn(async {
            let _ = check_for_updates().await;
        });
        return json_response(&cached_result);
    }
    
    // Otherwise, do a fresh check
    match check_for_updates().await {
        Ok(result) => json_response(&result),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e)
    }
}

async fn handle_process_model(body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct ModelProcessRequest {
        file_data: String, // base64 encoded file data
        filename: String,
        project_name: String,
    }
    
    let request: ModelProcessRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            error!("❌ Failed to parse model process request: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid JSON format");
        }
    };
    
    info!("🎨 Processing model: {} for project: {}", request.filename, request.project_name);
    
    // Decode base64 file data
    let file_data = match general_purpose::STANDARD.decode(&request.file_data) {
        Ok(data) => data,
        Err(e) => {
            error!("❌ Failed to decode base64 file data: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid base64 file data");
        }
    };
    
    // Extract settings from the model file itself and use intelligent defaults
    let extracted_settings = extract_model_settings(&file_data, &request.filename);
    
    // Process the model import
    match process_model_import(file_data, &request.filename, &request.project_name, extracted_settings) {
        Ok(result) => {
            info!("✅ Model processing completed: {}", request.filename);
            json_response(&result)
        }
        Err(e) => {
            error!("❌ Model processing failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &e)
        }
    }
}

async fn handle_update_model_summary(body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct UpdateSummaryRequest {
        summary_path: String,
        scene_analysis: SceneAnalysis,
    }
    
    let request: UpdateSummaryRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            error!("❌ Failed to parse update summary request: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid JSON format");
        }
    };
    
    info!("📊 Updating model summary with scene analysis: {}", request.summary_path);
    
    // Read existing summary
    let projects_path = get_projects_path();
    let summary_file_path = projects_path.join(&request.summary_path);
    
    let mut summary_data: serde_json::Value = match fs::read_to_string(&summary_file_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(data) => data,
            Err(e) => {
                error!("❌ Failed to parse existing summary JSON: {}", e);
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Invalid summary file format");
            }
        },
        Err(e) => {
            error!("❌ Failed to read summary file: {:?} - Error: {}", summary_file_path, e);
            return error_response(StatusCode::NOT_FOUND, "Summary file not found");
        }
    };
    
    // Update with scene analysis
    summary_data["scene_analysis"] = serde_json::to_value(&request.scene_analysis).unwrap();
    
    // Write back to file
    let updated_json = match serde_json::to_string_pretty(&summary_data) {
        Ok(json) => json,
        Err(e) => {
            error!("❌ Failed to serialize updated summary: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize summary");
        }
    };
    
    if let Err(e) = fs::write(&summary_file_path, updated_json) {
        error!("❌ Failed to write updated summary: {:?} - Error: {}", summary_file_path, e);
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to write updated summary");
    }
    
    info!("✅ Updated model summary with scene analysis: {}", request.summary_path);
    
    let response = ApiResponse {
        success: true,
        content: Some("Summary updated with scene analysis".to_string()),
        error: None,
    };
    json_response(&response)
}

async fn handle_compile_script(script_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    // Check Redis cache first
    if let Some(redis_cache) = REDIS_CACHE.get() {
        let mut cache = redis_cache.lock().await;
        if let Some(cached_js) = cache.get_cached_compiled_script(script_name) {
            info!("🔴 Cache hit for compiled script: {}", script_name);
            return Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/javascript")
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(BoxBody::new(Full::new(Bytes::from(cached_js))))
                .unwrap();
        }
    }
    
    // Cache miss - compile the script
    match compile_renscript(script_name) {
        Ok(compiled_js) => {
            // Cache the compiled result
            if let Some(redis_cache) = REDIS_CACHE.get() {
                let mut cache = redis_cache.lock().await;
                cache.cache_compiled_script(script_name, &compiled_js);
            }
            
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/javascript")
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(BoxBody::new(Full::new(Bytes::from(compiled_js))))
                .unwrap()
        }
        Err(e) => {
            error!("❌ Failed to compile script '{}': {}", script_name, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &e)
        }
    }
}

async fn handle_search_scripts(query: &str) -> Response<BoxBody<Bytes, Infallible>> {
    // Parse query parameters
    let search_term = match query.split('=').nth(1) {
        Some(term) => percent_decode_str(term).decode_utf8().unwrap_or_default().to_string(),
        None => String::new(),
    };
    
    // Check Redis cache first
    if let Some(redis_cache) = REDIS_CACHE.get() {
        let mut cache = redis_cache.lock().await;
        if let Some(cached_scripts) = cache.get_cached_script_list() {
            let filtered_scripts: Vec<_> = if search_term.is_empty() {
                cached_scripts
            } else {
                cached_scripts.into_iter()
                    .filter(|script| script.name.to_lowercase().contains(&search_term.to_lowercase()) 
                             || script.directory.to_lowercase().contains(&search_term.to_lowercase()))
                    .collect()
            };
            
            if !filtered_scripts.is_empty() {
                info!("🔴 Cache hit for script search: '{}' ({} results)", search_term, filtered_scripts.len());
                let response = ApiResponse {
                    success: true,
                    content: Some(serde_json::to_string(&filtered_scripts).unwrap()),
                    error: None,
                };
                return json_response(&response);
            }
        }
    }
    
    // Cache miss - search database
    let database = match DATABASE.get() {
        Some(db) => db,
        None => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Database not initialized");
        }
    };
    
    let results = if search_term.is_empty() {
        database.get_all_scripts().await
    } else {
        database.search_scripts(&search_term).await
    };
    
    match results {
        Ok(scripts) => {
            // Cache the results
            if let Some(redis_cache) = REDIS_CACHE.get() {
                let mut cache = redis_cache.lock().await;
                cache.cache_script_list(&scripts);
            }
            
            let response = ApiResponse {
                success: true,
                content: Some(serde_json::to_string(&scripts).unwrap()),
                error: None,
            };
            json_response(&response)
        }
        Err(e) => {
            error!("❌ Failed to search scripts: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Database search failed: {}", e))
        }
    }
}

async fn handle_list_scripts() -> Response<BoxBody<Bytes, Infallible>> {
    handle_search_scripts("").await
}

async fn handle_clear_script_cache() -> Response<BoxBody<Bytes, Infallible>> {
    if let Some(redis_cache) = REDIS_CACHE.get() {
        let mut cache = redis_cache.lock().await;
        let cleared = cache.clear_all_cache();
        
        let response = ApiResponse {
            success: true,
            content: Some(format!("Cache cleared: {}", cleared)),
            error: None,
        };
        json_response(&response)
    } else {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "Redis cache not initialized")
    }
}

async fn handle_cache_stats() -> Response<BoxBody<Bytes, Infallible>> {
    let mut stats = serde_json::json!({
        "redis": {
            "enabled": false,
            "status": "not_initialized"
        },
        "database": {
            "enabled": false,
            "status": "not_initialized"
        }
    });
    
    // Get Redis stats
    if let Some(redis_cache) = REDIS_CACHE.get() {
        let mut cache = redis_cache.lock().await;
        stats["redis"] = cache.get_cache_stats();
    }
    
    // Get Database stats
    if let Some(database) = DATABASE.get() {
        match database.get_compilation_stats().await {
            Ok(db_stats) => {
                stats["database"] = serde_json::json!({
                    "enabled": true,
                    "status": "connected",
                    "stats": db_stats
                });
            }
            Err(e) => {
                stats["database"] = serde_json::json!({
                    "enabled": true,
                    "status": "error",
                    "error": e.to_string()
                });
            }
        }
    }
    
    let response = ApiResponse {
        success: true,
        content: Some(stats.to_string()),
        error: None,
    };
    json_response(&response)
}

async fn handle_convert_to_glb(body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct ConvertToGlbRequest {
        file_data: String, // base64 encoded file data
        filename: String,
        project_name: String,
        settings: Option<serde_json::Value>, // Optional import settings
        import_mode: Option<String>, // "separate" or "combined", defaults to "separate"
    }
    let request: ConvertToGlbRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            error!("❌ Failed to parse convert-to-glb request: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid JSON format");
        }
    };
    
    info!("🔄 Converting {} to GLB with extraction for project: {}", request.filename, request.project_name);
    
    // Decode base64 file data
    let file_data = match general_purpose::STANDARD.decode(&request.file_data) {
        Ok(data) => data,
        Err(e) => {
            error!("❌ Failed to decode base64 file data: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid base64 file data");
        }
    };
    
    // Parse import mode
    let import_mode = match request.import_mode.as_deref() {
        Some("combined") => Some(ImportMode::Combined),
        Some("separate") => Some(ImportMode::Separate),
        None => Some(ImportMode::Separate), // Default to separate (Unreal-style)
        _ => Some(ImportMode::Separate),
    };
    
    // Parse compression settings
    let compression = if let Some(settings) = &request.settings {
        if let Some(materials) = settings.get("materials") {
            CompressionSettings {
                draco_compression: materials.get("dracoCompression").and_then(|v| v.as_bool()),
                tmf_encoding: materials.get("tmfEncoding").and_then(|v| v.as_bool()),
            }
        } else {
            CompressionSettings {
                draco_compression: Some(false),
                tmf_encoding: Some(false),
            }
        }
    } else {
        CompressionSettings {
            draco_compression: Some(false),
            tmf_encoding: Some(false),
        }
    };
    
    // Convert to GLB and extract assets
    match convert_model_to_glb_and_extract(file_data, &request.filename, &request.project_name, import_mode, Some(compression)) {
        Ok(result) => {
            info!("✅ GLB conversion and extraction completed: {}", request.filename);
            json_response(&result)
        }
        Err(e) => {
            error!("❌ GLB conversion and extraction failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &e)
        }
    }
}

// RenScript cache handlers
async fn handle_search_renscripts(query: &str) -> Response<BoxBody<Bytes, Infallible>> {
    // Parse query parameters
    let search_term = match query.split('=').nth(1) {
        Some(term) => percent_decode_str(term).decode_utf8().unwrap_or_default().to_string(),
        None => String::new(),
    };
    
    if let Some(renscript_cache) = RENSCRIPT_CACHE.get() {
        match renscript_cache.search(&search_term).await {
            Ok(results) => json_response(&results),
            Err(e) => {
                error!("❌ Failed to search renscripts: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to search renscripts")
            }
        }
    } else {
        error_response(StatusCode::SERVICE_UNAVAILABLE, "RenScript cache not available")
    }
}

async fn handle_list_renscripts() -> Response<BoxBody<Bytes, Infallible>> {
    if let Some(renscript_cache) = RENSCRIPT_CACHE.get() {
        match renscript_cache.get_all_scripts().await {
            Ok(scripts) => json_response(&scripts),
            Err(e) => {
                error!("❌ Failed to list renscripts: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list renscripts")
            }
        }
    } else {
        error_response(StatusCode::SERVICE_UNAVAILABLE, "RenScript cache not available")
    }
}

async fn handle_refresh_renscript_cache() -> Response<BoxBody<Bytes, Infallible>> {
    if let Some(renscript_cache) = RENSCRIPT_CACHE.get() {
        let renscripts_path = std::path::Path::new("renscripts");
        match renscript_cache.refresh_cache(renscripts_path).await {
            Ok(_) => {
                info!("✅ RenScript cache refreshed successfully");
                json_response(&serde_json::json!({"success": true, "message": "Cache refreshed"}))
            }
            Err(e) => {
                error!("❌ Failed to refresh renscript cache: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to refresh cache")
            }
        }
    } else {
        error_response(StatusCode::SERVICE_UNAVAILABLE, "RenScript cache not available")
    }
}

async fn handle_database_query(body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct QueryRequest {
        query: String,
    }

    let request: QueryRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            error!("❌ Failed to parse database query request: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid JSON format");
        }
    };

    if let Some(database) = DATABASE.get() {
        match database.execute_raw_query(&request.query).await {
            Ok(results) => {
                info!("✅ Database query executed successfully");
                json_response(&results)
            }
            Err(e) => {
                error!("❌ Database query failed: {}", e);
                error_response(StatusCode::BAD_REQUEST, &e.to_string())
            }
        }
    } else {
        error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not available")
    }
}


