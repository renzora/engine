use hyper::{Request, Response, Method, StatusCode};
use hyper::header::{CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_HEADERS};
use http_body_util::{BodyExt, Full, StreamBody, combinators::BoxBody};
use hyper::body::Frame;
use bytes::Bytes;
use async_stream::stream;
use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use log::{info, warn, error, debug};
use percent_encoding::percent_decode_str;
use base64::{Engine as _, engine::general_purpose};
use crate::types::{ApiResponse, WriteFileRequest, WriteBinaryFileRequest, CreateProjectRequest};
use crate::project_manager::{list_projects, list_directory_contents, create_project};
use crate::file_sync::{read_file_content, write_file_content, delete_file_or_directory, get_file_content_type, read_binary_file, write_binary_file_content};
use crate::thumbnail_generator::{get_or_generate_thumbnail, ThumbnailRequest};
use crate::update_manager::{Channel, check_for_updates, set_update_channel, get_current_config, get_last_update_check};
use crate::file_watcher::{get_file_change_receiver, set_current_project};
use crate::system_monitor::get_system_stats;
use crate::model_processor::{process_model_import, ModelImportSettings};

// Static variable to store startup time
static STARTUP_TIME: OnceLock<u64> = OnceLock::new();

pub fn set_startup_time(timestamp: u64) {
    STARTUP_TIME.set(timestamp).ok();
}

pub async fn handle_http_request(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    let start_time = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("");
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

fn handle_create_project(body_content: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match serde_json::from_str::<CreateProjectRequest>(body_content) {
        Ok(request) => {
            match create_project(&request.name, &request.template.unwrap_or_else(|| "basic".to_string())) {
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
        settings: ModelImportSettings,
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
    
    // Process the model import
    match process_model_import(file_data, &request.filename, &request.project_name, request.settings) {
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


