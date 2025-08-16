use hyper::{Request, Response, Method, StatusCode};
use hyper::header::{CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_HEADERS};
use http_body_util::{BodyExt, Full};
use bytes::Bytes;
use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::types::{ApiResponse, WriteFileRequest, WriteBinaryFileRequest, CreateProjectRequest};
use crate::project_manager::{list_projects, list_directory_contents, create_project};
use crate::file_sync::{read_file_content, write_file_content, delete_file_or_directory, get_file_content_type, read_binary_file, write_binary_file_content};
use crate::thumbnail_generator::{get_or_generate_thumbnail, ThumbnailRequest};
use crate::update_manager::{Channel, check_for_updates, set_update_channel, get_current_config, get_last_update_check};

// Static variable to store startup time
static STARTUP_TIME: OnceLock<u64> = OnceLock::new();

pub fn set_startup_time(timestamp: u64) {
    STARTUP_TIME.set(timestamp).ok();
}

pub async fn handle_http_request(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    
    // Handle CORS preflight
    if method == Method::OPTIONS {
        return Ok(cors_response(StatusCode::OK, ""));
    }
    
    // Special handling for SSE streaming endpoint - return simple response for now
    if method == Method::GET && path == "/file-changes/stream" {
        return Ok(simple_sse_response());
    }
    
    // Read body if this is a POST request
    let body = if method == Method::POST {
        match read_request_body(req).await {
            Ok(body) => Some(body),
            Err(_) => return Ok(error_response(StatusCode::BAD_REQUEST, "Failed to read request body")),
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
            handle_list_directory(dir_path)
        }
        (&Method::GET, path) if path.starts_with("/read/") => {
            let file_path = &path[6..];
            handle_read_file(file_path)
        }
        (&Method::POST, path) if path.starts_with("/write/") => {
            let file_path = &path[7..];
            match &body {
                Some(body_content) => handle_write_file(file_path, body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, path) if path.starts_with("/write-binary/") => {
            let file_path = &path[14..];
            match &body {
                Some(body_content) => handle_write_binary_file(file_path, body_content),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::DELETE, path) if path.starts_with("/delete/") => {
            let file_path = &path[8..];
            handle_delete_file(file_path)
        }
        (&Method::GET, path) if path.starts_with("/file/") => {
            let file_path = &path[6..];
            return Ok(handle_serve_asset(file_path));
        }
        (&Method::POST, "/start-watcher") => handle_start_watcher(),
        (&Method::GET, "/file-changes") => handle_get_file_changes(),
        (&Method::POST, "/clear-changes") => handle_clear_file_changes(),
        (&Method::POST, "/thumbnail") => {
            match &body {
                Some(body_content) => handle_generate_thumbnail(body_content).await,
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::GET, "/health") => handle_health_check(),
        (&Method::GET, "/startup-time") => handle_get_startup_time(),
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
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    };
    
    Ok(response)
}

async fn read_request_body(req: Request<hyper::body::Incoming>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = req.into_body();
    let body_bytes = body.collect().await?.to_bytes();
    Ok(String::from_utf8(body_bytes.to_vec())?)
}

fn cors_response(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, DELETE, OPTIONS")
        .header(ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn json_response<T: serde::Serialize>(data: &T) -> Response<Full<Bytes>> {
    let json = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    cors_response(StatusCode::OK, &json)
}

fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    let error_response = ApiResponse {
        success: false,
        content: None,
        error: Some(message.to_string()),
    };
    let json = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
    cors_response(status, &json)
}

fn handle_get_projects() -> Response<Full<Bytes>> {
    match list_projects() {
        Ok(projects) => json_response(&projects),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn handle_list_directory(dir_path: &str) -> Response<Full<Bytes>> {
    match list_directory_contents(dir_path) {
        Ok(files) => json_response(&files),
        Err(e) => error_response(StatusCode::FORBIDDEN, &e),
    }
}

fn handle_read_file(file_path: &str) -> Response<Full<Bytes>> {
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

fn handle_write_file(file_path: &str, body: &str) -> Response<Full<Bytes>> {
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

fn handle_write_binary_file(file_path: &str, body: &str) -> Response<Full<Bytes>> {
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

fn handle_delete_file(file_path: &str) -> Response<Full<Bytes>> {
    match delete_file_or_directory(file_path) {
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

fn handle_serve_asset(file_path: &str) -> Response<Full<Bytes>> {
    match read_binary_file(file_path) {
        Ok(contents) => {
            let full_path = std::path::Path::new(file_path);
            let content_type = get_file_content_type(full_path);
            
            Response::builder()
                .status(StatusCode::OK)
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(CONTENT_TYPE, content_type)
                .body(Full::new(Bytes::from(contents)))
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

fn handle_start_watcher() -> Response<Full<Bytes>> {
    let response = ApiResponse {
        success: true,
        content: Some("File watcher is running".to_string()),
        error: None,
    };
    json_response(&response)
}

fn handle_get_file_changes() -> Response<Full<Bytes>> {
    json_response(&Vec::<String>::new())
}

fn handle_clear_file_changes() -> Response<Full<Bytes>> {
    let response = ApiResponse {
        success: true,
        content: None,
        error: None,
    };
    json_response(&response)
}

async fn handle_generate_thumbnail(body_content: &str) -> Response<Full<Bytes>> {
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

fn handle_create_project(body_content: &str) -> Response<Full<Bytes>> {
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

fn simple_sse_response() -> Response<Full<Bytes>> {
    let sse_data = "data: {\"type\":\"connected\",\"message\":\"File change stream connected\"}\n\n";
    
    Response::builder()
        .status(StatusCode::OK)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, DELETE, OPTIONS")
        .header(ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .header(CONTENT_TYPE, "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Full::new(Bytes::from(sse_data)))
        .unwrap()
}

fn handle_health_check() -> Response<Full<Bytes>> {
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

fn handle_get_startup_time() -> Response<Full<Bytes>> {
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

fn handle_restart_bridge() -> Response<Full<Bytes>> {
    // In a real implementation, this would restart the server
    // For now, just return success
    let response = ApiResponse {
        success: true,
        content: Some("Bridge restart initiated".to_string()),
        error: None,
    };
    json_response(&response)
}

fn handle_clear_cache() -> Response<Full<Bytes>> {
    // In a real implementation, this would clear caches
    // For now, just return success
    let response = ApiResponse {
        success: true,
        content: Some("Cache cleared successfully".to_string()),
        error: None,
    };
    json_response(&response)
}


fn handle_set_update_channel(body: &str) -> Response<Full<Bytes>> {
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

fn handle_get_update_config() -> Response<Full<Bytes>> {
    let config = get_current_config();
    json_response(&config)
}

async fn handle_check_updates() -> Response<Full<Bytes>> {
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


