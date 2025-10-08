use hyper::{Request, Response, Method, StatusCode};
use hyper::header::{CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_HEADERS};
use http_body_util::{BodyExt, Full, StreamBody, combinators::BoxBody};
use hyper::body::Frame;
use bytes::Bytes;
use async_stream::stream;
use std::convert::Infallible;
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use log::{info, warn, error, debug};
use percent_encoding::percent_decode_str;
use base64::{Engine as _, engine::general_purpose};
use crate::types::{ApiResponse, WriteFileRequest, WriteBinaryFileRequest, CreateProjectRequest};
use crate::project_manager::{list_projects, list_directory_contents, create_project, load_scene_with_assets, delete_project};
use crate::file_sync::{read_file_content, write_file_content, delete_file_or_directory, get_file_content_type, read_binary_file, write_binary_file_content};
use crate::update_manager::{Channel, check_for_updates, set_update_channel, get_current_config, get_last_update_check};
use crate::file_watcher::{get_file_change_receiver, set_current_project};
use crate::system_monitor::get_system_stats;
use crate::model_processor::{process_model_import, extract_model_settings, SceneAnalysis};
use crate::model_converter::{convert_model_to_glb_and_extract, ImportMode, CompressionSettings};
use crate::project_manager::get_projects_path;
use crate::renscript_compiler::compile_renscript;
use std::fs;
use tokio::fs as tokio_fs;
// Database removed - using Redis-only caching
use crate::modules::memory_cache::{MemoryCache, CachedAssetNode, ProjectAssetTree};
use crate::renscript_cache::RenScriptCache;

// Unified file operations
#[derive(Debug)]
enum FileOperation {
    List,
    Read,
    Write { content: String },
    WriteBinary { content: String },
    Delete,
    Serve,
}

#[derive(Debug)]
struct UnifiedFileRequest {
    path: String,
    operation: FileOperation,
    accept_header: Option<String>,
}

// Static variables for shared state
static STARTUP_TIME: OnceLock<u64> = OnceLock::new();
// Database removed - using Redis-only caching
static MEMORY_CACHE: OnceLock<Arc<tokio::sync::Mutex<MemoryCache>>> = OnceLock::new();
static RENSCRIPT_CACHE: OnceLock<Arc<RenScriptCache>> = OnceLock::new();

pub fn set_startup_time(timestamp: u64) {
    STARTUP_TIME.set(timestamp).ok();
}

// Database functions removed - using Redis-only caching

pub fn set_memory_cache(memory_cache: Arc<tokio::sync::Mutex<MemoryCache>>) {
    MEMORY_CACHE.set(memory_cache).ok();
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
    let accept_header = req.headers().get("accept")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    
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
        (&Method::DELETE, path) if path.starts_with("/projects/") => {
            let project_name = &path[10..];
            let decoded_name = match decode_url_path(project_name) {
                Ok(name) => name,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_delete_project(&decoded_name)
        }
        (&Method::GET, path) if path.contains("/cache/validate") && path.starts_with("/projects/") => {
            // Extract project name from path: /projects/{name}/cache/validate
            let path_parts: Vec<&str> = path.split('/').collect();
            if path_parts.len() >= 4 && path_parts[1] == "projects" && path_parts[3] == "cache" && path_parts[4] == "validate" {
                let project_name = path_parts[2];
                let decoded_name = match decode_url_path(project_name) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                handle_validate_project_cache(&decoded_name).await
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid cache validation path")
            }
        }
        (&Method::POST, path) if path.contains("/cache/process") && path.starts_with("/projects/") => {
            // Extract project name from path: /projects/{name}/cache/process
            let path_parts: Vec<&str> = path.split('/').collect();
            if path_parts.len() >= 4 && path_parts[1] == "projects" && path_parts[3] == "cache" && path_parts[4] == "process" {
                let project_name = path_parts[2];
                let decoded_name = match decode_url_path(project_name) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                match &body {
                    Some(body_content) => handle_process_project_cache(&decoded_name, body_content).await,
                    None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
                }
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid cache process path")
            }
        }
        (&Method::GET, path) if path.contains("/cache/tree") && path.starts_with("/projects/") => {
            // Extract project name from path: /projects/{name}/cache/tree
            let path_parts: Vec<&str> = path.split('/').collect();
            if path_parts.len() >= 4 && path_parts[1] == "projects" && path_parts[3] == "cache" && path_parts[4] == "tree" {
                let project_name = path_parts[2];
                let decoded_name = match decode_url_path(project_name) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                handle_get_asset_tree(&decoded_name).await
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid asset tree path")
            }
        }
        (&Method::GET, path) if path.contains("/assets") && path.starts_with("/projects/") => {
            // Extract project name from path: /projects/{name}/assets
            let path_parts: Vec<&str> = path.split('/').collect();
            if path_parts.len() >= 3 && path_parts[1] == "projects" && path_parts[3] == "assets" {
                let project_name = path_parts[2];
                let decoded_name = match decode_url_path(project_name) {
                    Ok(name) => name,
                    Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
                };
                handle_get_cached_assets(&decoded_name).await
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid assets path")
            }
        }
        // Unified file operations
        (&Method::GET, path) if path.starts_with("/list/") => {
            let file_path = &path[6..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_unified_file_operation(UnifiedFileRequest {
                path: decoded_path,
                operation: FileOperation::List,
                accept_header: accept_header.clone(),
            })
        }
        (&Method::GET, path) if path.starts_with("/read/") => {
            let file_path = &path[6..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_unified_file_operation(UnifiedFileRequest {
                path: decoded_path,
                operation: FileOperation::Read,
                accept_header: accept_header.clone(),
            })
        }
        (&Method::POST, path) if path.starts_with("/write/") => {
            let file_path = &path[7..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            match &body {
                Some(body_content) => handle_unified_file_operation(UnifiedFileRequest {
                    path: decoded_path,
                    operation: FileOperation::Write { content: body_content.clone() },
                    accept_header: accept_header.clone(),
                }),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::POST, path) if path.starts_with("/write-binary/") => {
            let file_path = &path[14..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            match &body {
                Some(body_content) => handle_unified_file_operation(UnifiedFileRequest {
                    path: decoded_path,
                    operation: FileOperation::WriteBinary { content: body_content.clone() },
                    accept_header: accept_header.clone(),
                }),
                None => error_response(StatusCode::BAD_REQUEST, "Missing request body"),
            }
        }
        (&Method::DELETE, path) if path.starts_with("/delete/") => {
            let file_path = &path[8..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            handle_unified_file_operation(UnifiedFileRequest {
                path: decoded_path,
                operation: FileOperation::Delete,
                accept_header: accept_header.clone(),
            })
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
        // Database query endpoint removed - using Redis-only caching
        (&Method::GET, path) if path.starts_with("/file/") => {
            let file_path = &path[6..];
            let decoded_path = match decode_and_validate_path(file_path) {
                Ok(path) => path,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &e)),
            };
            return Ok(handle_unified_file_operation(UnifiedFileRequest {
                path: decoded_path,
                operation: FileOperation::Serve,
                accept_header: accept_header.clone(),
            }));
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

// Old file handlers removed - using unified file handler

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
            // Auto-process files based on type
            let file_path_lower = file_path.to_lowercase();
            let path_parts: Vec<&str> = file_path.split('/').collect();
            
            if path_parts.len() >= 2 {
                let _project_name = path_parts[0];
                
                // HDR/EXR files are now handled directly by BabylonJS - no processing needed
                if file_path_lower.ends_with(".hdr") || file_path_lower.ends_with(".exr") {
                    println!("🌍 HDR/EXR file uploaded: {} (will be handled by native BabylonJS support)", file_path);
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

// Unified smart file handler
fn handle_unified_file_operation(request: UnifiedFileRequest) -> Response<BoxBody<Bytes, Infallible>> {
    match request.operation {
        FileOperation::List => handle_list_operation(&request.path),
        FileOperation::Read => handle_read_operation(&request.path, request.accept_header.as_deref()),
        FileOperation::Write { content } => handle_write_operation(&request.path, &content, false),
        FileOperation::WriteBinary { content } => handle_write_operation(&request.path, &content, true),
        FileOperation::Delete => handle_delete_operation(&request.path),
        FileOperation::Serve => handle_serve_operation(&request.path),
    }
}

fn handle_list_operation(dir_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match list_directory_contents(dir_path) {
        Ok(files) => json_response(&files),
        Err(e) => error_response(StatusCode::FORBIDDEN, &e),
    }
}

fn handle_read_operation(file_path: &str, accept_header: Option<&str>) -> Response<BoxBody<Bytes, Infallible>> {
    // Check if client wants raw content (e.g., Accept: application/octet-stream)
    let wants_raw = accept_header
        .map(|h| h.contains("application/octet-stream") || h.contains("*/*"))
        .unwrap_or(false);
    
    if wants_raw || is_likely_binary_file(file_path) {
        // Serve as binary
        handle_serve_operation(file_path)
    } else {
        // Read as text and return JSON
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
}

fn handle_write_operation(file_path: &str, content: &str, is_binary: bool) -> Response<BoxBody<Bytes, Infallible>> {
    let result = if is_binary {
        let write_req: WriteBinaryFileRequest = match serde_json::from_str(content) {
            Ok(req) => req,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON"),
        };
        write_binary_file_content(file_path, &write_req)
    } else {
        let write_req: WriteFileRequest = match serde_json::from_str(content) {
            Ok(req) => req,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON"),
        };
        write_file_content(file_path, &write_req)
    };

    match result {
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

fn handle_delete_operation(file_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
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

fn handle_serve_operation(file_path: &str) -> Response<BoxBody<Bytes, Infallible>> {
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

// Helper function to detect binary files based on extension
fn is_likely_binary_file(file_path: &str) -> bool {
    let path = std::path::Path::new(file_path);
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(ext.to_lowercase().as_str(), 
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" |
            "mp4" | "mov" | "avi" | "mkv" | "webm" |
            "mp3" | "wav" | "ogg" | "flac" |
            "zip" | "rar" | "7z" | "tar" | "gz" |
            "pdf" | "doc" | "docx" | "ppt" | "pptx" |
            "exe" | "dll" | "so" | "dylib" |
            "glb" | "gltf" | "fbx" | "obj" | "dae" | "blend" |
            "ttf" | "otf" | "woff" | "woff2"
        )
    } else {
        false
    }
}

// Helper function to decode and validate file paths
fn decode_and_validate_path(encoded_path: &str) -> Result<String, String> {
    decode_url_path(encoded_path)
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

fn handle_delete_project(project_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    match delete_project(project_name) {
        Ok(_) => {
            let response = ApiResponse {
                success: true,
                content: Some(format!("Project '{}' deleted successfully", project_name)),
                error: None,
            };
            json_response(&response)
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
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
    // Check memory cache first
    if let Some(redis_cache) = MEMORY_CACHE.get() {
        let cache = redis_cache.lock().await;
        if let Some(cached_js) = cache.get_cached_compiled_script(script_name).await {
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
            if let Some(redis_cache) = MEMORY_CACHE.get() {
                let cache = redis_cache.lock().await;
                cache.cache_compiled_script(script_name, &compiled_js).await;
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
    
    // Check memory cache first
    if let Some(redis_cache) = MEMORY_CACHE.get() {
        let cache = redis_cache.lock().await;
        if let Some(cached_scripts) = cache.get_cached_script_list().await {
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
    
    // Cache miss - use RenScript cache instead of database
    if let Some(renscript_cache) = RENSCRIPT_CACHE.get() {
        match if search_term.is_empty() {
            renscript_cache.get_all_scripts().await
        } else {
            renscript_cache.search(&search_term).await
        } {
            Ok(scripts) => {
                // Convert RenScript entries to the expected format
                let script_results: Vec<_> = scripts.into_iter().map(|script| {
                    serde_json::json!({
                        "name": script.name,
                        "path": script.path,
                        "directory": script.directory,
                        "last_modified": 0 // RenScript cache doesn't track modification time
                    })
                }).collect();
                
                let response = ApiResponse {
                    success: true,
                    content: Some(serde_json::to_string(&script_results).unwrap()),
                    error: None,
                };
                json_response(&response)
            }
            Err(e) => {
                error!("❌ Failed to search RenScript cache: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("RenScript cache search failed: {}", e))
            }
        }
    } else {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "RenScript cache not initialized")
    }
}

async fn handle_list_scripts() -> Response<BoxBody<Bytes, Infallible>> {
    handle_search_scripts("").await
}

async fn handle_clear_script_cache() -> Response<BoxBody<Bytes, Infallible>> {
    if let Some(redis_cache) = MEMORY_CACHE.get() {
        let cache = redis_cache.lock().await;
        let cleared = cache.clear_all_cache();
        
        let response = ApiResponse {
            success: true,
            content: Some(format!("Cache cleared: {}", cleared)),
            error: None,
        };
        json_response(&response)
    } else {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "Memory cache not initialized")
    }
}

async fn handle_cache_stats() -> Response<BoxBody<Bytes, Infallible>> {
    let mut stats = serde_json::json!({
        "memory_cache": {
            "enabled": false,
            "status": "not_initialized"
        },
        "database": {
            "enabled": false,
            "status": "not_initialized"
        }
    });
    
    // Get Memory Cache stats
    if let Some(memory_cache) = MEMORY_CACHE.get() {
        let cache = memory_cache.lock().await;
        stats["memory_cache"] = cache.get_cache_stats();
    }
    
    // Database removed - using Redis-only caching
    stats["database"] = serde_json::json!({
        "enabled": false,
        "status": "removed",
        "note": "Using lightweight memory-only caching for better performance"
    });
    
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
        current_path: Option<String>, // Current directory path, defaults to "assets"
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
    let current_path = request.current_path.as_deref();
    match convert_model_to_glb_and_extract(file_data, &request.filename, &request.project_name, import_mode, Some(compression), current_path) {
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

// Database query function removed - using Redis-only caching

// Project Cache Handlers

async fn handle_validate_project_cache(project_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    info!("🔍 Validating cache for project: {}", project_name);
    
    let redis_cache = MEMORY_CACHE.get().cloned();
    let validator = crate::modules::project_cache_validator::ProjectCacheValidator::new(
        project_name.to_string(),
        redis_cache,
    );
    
    match validator.validate_cache().await {
        Ok(validation_result) => {
            info!("✅ Cache validation completed for project: {} (status: {})", 
                  project_name, validation_result.cache_status);
            json_response(&validation_result)
        }
        Err(e) => {
            error!("❌ Cache validation failed for project {}: {}", project_name, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Cache validation failed: {}", e))
        }
    }
}

async fn handle_process_project_cache(project_name: &str, body: &str) -> Response<BoxBody<Bytes, Infallible>> {
    #[derive(serde::Deserialize)]
    struct ProcessCacheRequest {
        force_full_rebuild: Option<bool>,
        file_types: Option<Vec<String>>,
        stream_progress: Option<bool>,
    }
    
    let request: ProcessCacheRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            error!("❌ Failed to parse cache process request: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Invalid JSON format");
        }
    };
    
    info!("🔄 Starting cache processing for project: {} (force_rebuild: {}, stream: {})", 
          project_name, 
          request.force_full_rebuild.unwrap_or(false),
          request.stream_progress.unwrap_or(false));
    
    let redis_cache = MEMORY_CACHE.get().cloned();
    let validator = crate::modules::project_cache_validator::ProjectCacheValidator::new(
        project_name.to_string(),
        redis_cache.clone(),
    );
    
    match validator.validate_cache().await {
        Ok(validation_result) => {
            if validation_result.cache_status == "valid" && !request.force_full_rebuild.unwrap_or(false) {
                let response = serde_json::json!({
                    "success": true,
                    "message": "Cache is already up to date",
                    "processed_count": 0,
                    "cache_status": "valid"
                });
                return json_response(&response);
            }
            
            // Process the cache with progress tracking
            info!("🔄 Starting cache processing for project: {}", project_name);
            
            if request.stream_progress.unwrap_or(false) {
                // Return SSE stream with real-time progress
                handle_sse_cache_processing(&validator, project_name, request.force_full_rebuild.unwrap_or(false)).await
            } else {
                // Regular JSON response
                match process_project_assets(&validator, project_name, request.force_full_rebuild.unwrap_or(false), None).await {
                    Ok(processed_count) => {
                        info!("🎉 Cache processing completed successfully for project: {} ({} assets)", project_name, processed_count);
                        let response = serde_json::json!({
                            "success": true,
                            "message": "Cache processing completed successfully",
                            "processed_count": processed_count,
                            "cache_status": "updated"
                        });
                        json_response(&response)
                    }
                    Err(e) => {
                        error!("❌ Cache processing failed: {}", e);
                        error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Processing failed: {}", e))
                    }
                }
            }
        }
        Err(e) => {
            error!("❌ Cache validation failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Validation failed: {}", e))
        }
    }
}

async fn handle_get_cached_assets(project_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    info!("📦 Retrieving cached assets for project: {}", project_name);
    
    if let Some(redis_cache) = MEMORY_CACHE.get() {
        let cache = redis_cache.lock().await;
        let processed_assets = cache.get_all_processed_assets(project_name).await;
        let file_metadata = cache.get_all_file_metadata(project_name).await;
        let manifest = cache.get_project_manifest(project_name).await;
        
        info!("🔍 Retrieved from Redis cache: {} processed assets, {} file metadata entries, manifest: {}", 
              processed_assets.len(), file_metadata.len(), manifest.is_some());
        
        let response_data = serde_json::json!({
            "success": true,
            "project_name": project_name,
            "assets": processed_assets,
            "file_metadata": file_metadata,
            "cache_generated_at": manifest.as_ref().map(|m| m.last_scan).unwrap_or(0),
            "total_processed": processed_assets.len(),
            "cache_version": manifest.as_ref().map(|m| m.cache_version.as_str()).unwrap_or("unknown")
        });
        
        info!("✅ Retrieved {} cached assets for project: {}", processed_assets.len(), project_name);
        json_response(&response_data)
    } else {
        error!("❌ Redis cache not available");
        error_response(StatusCode::SERVICE_UNAVAILABLE, "Cache not available")
    }
}

async fn handle_get_asset_tree(project_name: &str) -> Response<BoxBody<Bytes, Infallible>> {
    info!("🌳 Retrieving cached asset tree for project: {}", project_name);
    
    if let Some(redis_cache) = MEMORY_CACHE.get() {
        let cache = redis_cache.lock().await;
        if let Some(asset_tree) = cache.get_project_asset_tree(project_name).await {
            info!("✅ Retrieved cached asset tree for project: {} ({} files, {} directories)", 
                  project_name, asset_tree.total_files, asset_tree.total_directories);
            json_response(&asset_tree)
        } else {
            warn!("🌳 No cached asset tree found for project: {}", project_name);
            error_response(StatusCode::NOT_FOUND, "Asset tree not found in cache")
        }
    } else {
        error!("❌ Redis cache not available");
        error_response(StatusCode::SERVICE_UNAVAILABLE, "Cache not available")
    }
}

// Progress callback type
type ProgressCallback = Box<dyn Fn(serde_json::Value) + Send + Sync>;

async fn handle_sse_cache_processing(
    _validator: &crate::modules::project_cache_validator::ProjectCacheValidator,
    project_name: &str,
    force_rebuild: bool,
) -> Response<BoxBody<Bytes, Infallible>> {
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio::sync::mpsc;
    use std::convert::Infallible;
    use futures_util::StreamExt;
    
    let (tx, rx) = mpsc::unbounded_channel();
    
    // Clone tx for the async task
    let tx_for_task = tx.clone();
    
    // Create progress callback that sends SSE events
    let progress_callback: ProgressCallback = Box::new(move |progress_data| {
        let event = format!("data: {}\n\n", progress_data);
        let _ = tx.send(Ok::<_, Infallible>(event.into()));
    });
    
    // Clone data for async task  
    let project_name_clone = project_name.to_string();
    let redis_cache = MEMORY_CACHE.get().cloned();
    let validator_for_task = crate::modules::project_cache_validator::ProjectCacheValidator::new(
        project_name_clone.clone(),
        redis_cache,
    );
    
    // Spawn processing task
    tokio::spawn(async move {
        let result = process_project_assets(&validator_for_task, &project_name_clone, force_rebuild, Some(progress_callback)).await;
        
        // Send final result
        let final_data = match result {
            Ok(processed_count) => serde_json::json!({
                "type": "complete",
                "success": true,
                "processed_count": processed_count,
                "message": "Cache processing completed successfully"
            }),
            Err(e) => serde_json::json!({
                "type": "error",
                "success": false,
                "error": e.to_string()
            })
        };
        
        let final_event = format!("data: {}\n\n", final_data);
        let _ = tx_for_task.send(Ok::<_, Infallible>(final_event.into()));
    });
    
    // Return SSE response
    let stream = UnboundedReceiverStream::new(rx);
    let body = StreamBody::new(stream.map(|item| {
        item.map(|data: String| Frame::data(Bytes::from(data)))
    }));
    
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .body(BodyExt::boxed(body))
        .unwrap()
}

// Helper function to process a single file
async fn process_single_file(
    file_path: std::path::PathBuf,
    project_path: &std::path::Path,
    project_name: &str,
    current_time: u64,
    file_number: usize,
    total_files: usize,
    progress_callback: Option<&ProgressCallback>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::modules::memory_cache::{FileMetadata, ProcessedAsset};
    use std::hash::{Hash, Hasher};
    
    if let Ok(relative_path) = file_path.strip_prefix(project_path) {
        let relative_path_str = relative_path.to_string_lossy().replace('\\', "/");
        let overall_progress = (file_number as f32 / total_files as f32) * 0.8; // Reserve 20% for finalization
        
        info!("📄 Processing file {}/{}: {}", file_number, total_files, relative_path_str);
        
        // Send progress update for each file
        if let Some(callback) = progress_callback {
            callback(serde_json::json!({
                "type": "progress",
                "stage": format!("Processing file {}/{}", file_number, total_files),
                "progress": overall_progress,
                "files_processed": file_number,
                "total_files": total_files,
                "current_file": relative_path_str
            }));
        }
        
        // Create file metadata
        if let Ok(metadata) = tokio_fs::metadata(&file_path).await {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            relative_path_str.hash(&mut hasher);
            metadata.len().hash(&mut hasher);
            
            let file_metadata = FileMetadata {
                path: relative_path_str.clone(),
                last_modified: metadata.modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                file_size: metadata.len(),
                hash: format!("{:x}", hasher.finish()),
                processed_at: current_time,
                file_type: get_file_type(&file_path),
            };
            
            // Cache the file metadata
            if let Some(redis_cache) = MEMORY_CACHE.get() {
                let cache = redis_cache.lock().await;
                if cache.cache_file_metadata(project_name, &[file_metadata.clone()]).await {
                    info!("📦 Cached file metadata: {} ({})", relative_path_str, file_metadata.file_type);
                }
            }
            
            // Process specific file types with detailed logging
            let file_type = get_file_type(&file_path);
            let processed_asset = match file_type.as_str() {
                "image" | "hdr_image" => {
                    info!("🖼️ Generating thumbnail for {}: {}", file_type, relative_path_str);
                    process_image_asset(project_name, &relative_path_str, &file_path).await?
                }
                "model" => {
                    info!("🎨 Processing 3D model: {}", relative_path_str);
                    process_model_asset(project_name, &relative_path_str, &file_path).await?
                }
                "audio" => {
                    info!("🎵 Processing audio file: {}", relative_path_str);
                    process_audio_asset(project_name, &relative_path_str, &file_path).await?
                }
                _ => {
                    info!("📄 Processing {} file: {}", file_type, relative_path_str);
                    // Generic processing for other file types
                    ProcessedAsset {
                        path: relative_path_str.clone(),
                        file_type: file_metadata.file_type.clone(),
                        metadata: serde_json::json!({
                            "size": metadata.len(),
                            "extension": file_path.extension()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default()
                        }),
                        thumbnail_path: None,
                        compressed_path: None,
                        extracted_materials: None,
                        processing_status: "processed".to_string(),
                        processed_at: current_time,
                    }
                }
            };
            
            // Cache the processed asset
            if let Some(redis_cache) = MEMORY_CACHE.get() {
                let cache = redis_cache.lock().await;
                if cache.cache_processed_asset(project_name, &processed_asset).await {
                    info!("💾 Cached processed asset: {} (status: {})", relative_path_str, processed_asset.processing_status);
                }
            }
            
            info!("✅ Completed processing file {}/{}: {} ({})", 
                  file_number, 
                  total_files, 
                  relative_path_str,
                  processed_asset.processing_status);
        }
    }
    Ok(())
}

async fn process_project_assets(
    validator: &crate::modules::project_cache_validator::ProjectCacheValidator,
    project_name: &str,
    force_rebuild: bool,
    progress_callback: Option<ProgressCallback>,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    // Scan all project files
    let projects_path = crate::get_projects_path();
    let project_path = projects_path.join(project_name);
    let mut current_files = Vec::new();
    scan_directory_recursive(&project_path, &mut current_files)?;
    
    // Filter out cache, system files, and configuration files
    current_files.retain(|path| {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        
        // Exclude system and cache files
        if path_str.contains(".cache") || 
           path_str.contains(".git") || 
           path_str.starts_with('.') ||
           !path.is_file() {
            return false;
        }
        
        // Exclude configuration files that don't need processing
        if file_name == "project.json" ||
           file_name == "package.json" ||
           file_name == "tsconfig.json" ||
           file_name == "webpack.config.js" ||
           ((path_str.contains("scenes/") || path_str.contains("scenes\\")) && file_name.ends_with(".json")) {
            info!("⏭️ Skipping configuration file: {}", path_str);
            return false;
        }
        
        true
    });
    
    if force_rebuild {
        // Clear existing cache
        if let Some(redis_cache) = MEMORY_CACHE.get() {
            let cache = redis_cache.lock().await;
            cache.clear_project_cache(project_name);
            info!("🗑️ Cleared existing cache for force rebuild");
        }
    }
    
    let mut processed_count = 0;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    
    // Process files in batches to avoid overwhelming the system
    let batch_size = 10;
    let total_batches = (current_files.len() + batch_size - 1) / batch_size;
    
    info!("🔄 Starting batch processing: {} total files in {} batches", current_files.len(), total_batches);
    
    // Send initial progress
    if let Some(ref callback) = progress_callback {
        callback(serde_json::json!({
            "type": "progress",
            "stage": "Starting file processing...",
            "progress": 0.05,
            "files_processed": 0,
            "total_files": current_files.len(),
            "current_file": ""
        }));
    }
    
    // Process files in parallel batches using FuturesUnordered for better performance
    for (batch_index, batch) in current_files.chunks(batch_size).enumerate() {
        info!("📦 Processing batch {}/{} ({} files) in parallel", batch_index + 1, total_batches, batch.len());
        
        let mut futures: FuturesUnordered<_> = batch.iter().enumerate().map(|(file_index, file_path)| {
            let file_number = batch_index * batch_size + file_index + 1;
            process_single_file(
                file_path.clone(),
                &project_path,
                project_name,
                current_time,
                file_number,
                current_files.len(),
                progress_callback.as_ref(),
            )
        }).collect();
        
        // Process all files in the batch concurrently
        while let Some(result) = futures.next().await {
            match result {
                Ok(_) => processed_count += 1,
                Err(e) => {
                    error!("❌ Failed to process file in batch {}: {}", batch_index + 1, e);
                    // Continue processing other files even if one fails
                }
            }
        }
        
        info!("✅ Completed parallel batch {}/{}", batch_index + 1, total_batches);
        
        // Small delay between batches to prevent overwhelming the system
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await; // Reduced delay since we're using parallel processing
    }
    
    // Update project manifest with detailed progress
    info!("📋 Finalizing cache - calculating project checksum for {} files...", current_files.len());
    
    // Send finalization progress
    if let Some(ref callback) = progress_callback {
        callback(serde_json::json!({
            "type": "progress",
            "stage": "📋 Finalizing cache - calculating checksums...",
            "progress": 0.9,
            "files_processed": current_files.len(),
            "total_files": current_files.len(),
            "current_file": "",
            "operation": "finalization"
        }));
    }
    
    validator.update_project_manifest(&current_files).await?;
    info!("📋 Project manifest updated successfully");
    
    // Build and cache project asset tree
    info!("🌳 Building project asset tree...");
    if let Some(ref callback) = progress_callback {
        callback(serde_json::json!({
            "type": "progress",
            "stage": "🌳 Building project asset tree...",
            "progress": 0.95,
            "files_processed": current_files.len(),
            "total_files": current_files.len(),
            "current_file": "",
            "operation": "building_tree"
        }));
    }
    
    build_and_cache_asset_tree(project_name).await?;
    info!("🌳 Project asset tree cached successfully");
    
    // Send completion progress
    if let Some(ref callback) = progress_callback {
        callback(serde_json::json!({
            "type": "progress",
            "stage": "✅ Cache processing completed!",
            "progress": 1.0,
            "files_processed": current_files.len(),
            "total_files": current_files.len(),
            "current_file": "",
            "operation": "complete"
        }));
    }
    
    info!("✅ Processed {} assets for project: {} - PROCESSING COMPLETE", processed_count, project_name);
    Ok(processed_count)
}

fn scan_directory_recursive(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Skip hidden and cache directories
                if let Some(dir_name) = path.file_name() {
                    let dir_str = dir_name.to_string_lossy();
                    if !dir_str.starts_with('.') {
                        scan_directory_recursive(&path, files)?;
                    }
                }
            } else {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn format_file_size(file_path: &std::path::Path) -> String {
    if let Ok(metadata) = std::fs::metadata(file_path) {
        let size = metadata.len();
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        }
    } else {
        "Unknown size".to_string()
    }
}

fn get_file_type(file_path: &std::path::Path) -> String {
    if let Some(extension) = file_path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tga" | "tiff" | "ico" | "svg" => "image",
            "hdr" | "exr" => "hdr_image",
            "glb" | "gltf" | "obj" | "fbx" | "dae" | "3ds" | "blend" | "stl" | "ply" => "model",
            "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio",
            "mp4" | "avi" | "mov" | "mkv" | "webm" | "wmv" => "video",
            "js" | "ts" | "jsx" | "tsx" => "script",
            "json" | "xml" | "yaml" | "yml" => "data",
            "txt" | "md" | "rst" => "document",
            "ren" => "renscript",
            _ => "other",
        }
    } else {
        "other"
    }.to_string()
}

async fn process_image_asset(
    _project_name: &str,
    relative_path: &str,
    file_path: &std::path::Path,
) -> Result<crate::modules::memory_cache::ProcessedAsset, Box<dyn std::error::Error + Send + Sync>> {
    use crate::modules::memory_cache::ProcessedAsset;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    
    // Get file extension for more specific logging
    let _extension = file_path.extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    
    // Thumbnails removed - images will be loaded directly
    let thumbnail_path: Option<String> = None;
    
    // Get image metadata
    let metadata = tokio_fs::metadata(file_path).await?;
    let asset_metadata = serde_json::json!({
        "size": metadata.len(),
        "width": null, // Could be extracted with image crate
        "height": null,
        "format": file_path.extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        "is_hdr": relative_path.ends_with(".hdr") || relative_path.ends_with(".exr")
    });
    
    Ok(ProcessedAsset {
        path: relative_path.to_string(),
        file_type: "image".to_string(),
        metadata: asset_metadata,
        thumbnail_path,
        compressed_path: None,
        extracted_materials: None,
        processing_status: "processed".to_string(),
        processed_at: current_time,
    })
}

async fn process_model_asset(
    _project_name: &str,
    relative_path: &str,
    file_path: &std::path::Path,
) -> Result<crate::modules::memory_cache::ProcessedAsset, Box<dyn std::error::Error + Send + Sync>> {
    use crate::modules::memory_cache::ProcessedAsset;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    
    // Thumbnails removed - models will use icons instead
    let thumbnail_path: Option<String> = None;
    
    // Get model metadata
    let metadata = tokio_fs::metadata(file_path).await?;
    let asset_metadata = serde_json::json!({
        "size": metadata.len(),
        "format": file_path.extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        "vertex_count": null, // Could be extracted with model parsing
        "triangle_count": null,
        "material_count": null
    });
    
    let processed_asset = ProcessedAsset {
        path: relative_path.to_string(),
        file_type: "model".to_string(),
        metadata: asset_metadata,
        thumbnail_path: thumbnail_path.clone(),
        compressed_path: None, // Could implement Draco compression here
        extracted_materials: None, // Could extract material list here
        processing_status: "processed".to_string(),
        processed_at: current_time,
    };
    
    info!("📦 ProcessedAsset created - path: {}, thumbnail_path: {:?}", relative_path, thumbnail_path);
    
    Ok(processed_asset)
}

async fn process_audio_asset(
    _project_name: &str,
    relative_path: &str,
    file_path: &std::path::Path,
) -> Result<crate::modules::memory_cache::ProcessedAsset, Box<dyn std::error::Error + Send + Sync>> {
    use crate::modules::memory_cache::ProcessedAsset;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    
    // Get audio metadata
    let metadata = tokio_fs::metadata(file_path).await?;
    let asset_metadata = serde_json::json!({
        "size": metadata.len(),
        "format": file_path.extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        "duration": null, // Could be extracted with audio parsing crate
        "sample_rate": null,
        "channels": null,
        "bitrate": null
    });
    
    Ok(ProcessedAsset {
        path: relative_path.to_string(),
        file_type: "audio".to_string(),
        metadata: asset_metadata,
        thumbnail_path: None, // Could generate waveform visualization
        compressed_path: None,
        extracted_materials: None,
        processing_status: "processed".to_string(),
        processed_at: current_time,
    })
}

async fn build_and_cache_asset_tree(project_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    info!("🌳 Building asset tree for project: {}", project_name);
    
    let projects_path = crate::get_projects_path();
    let project_path = projects_path.join(project_name);
    let assets_path = project_path.join("assets");
    
    if !assets_path.exists() {
        warn!("🌳 Assets directory not found for project: {}", project_name);
        return Ok(());
    }
    
    // Get Redis cache for thumbnail URLs
    let redis_cache = MEMORY_CACHE.get();
    let processed_assets = if let Some(cache) = redis_cache {
        let cache_lock = cache.lock().await;
        cache_lock.get_all_processed_assets(project_name).await
    } else {
        Vec::new()
    };
    
    // Create a map of asset paths to thumbnail URLs for quick lookup
    let mut thumbnail_map = std::collections::HashMap::new();
    for asset in processed_assets {
        if let Some(thumbnail_path) = asset.thumbnail_path {
            thumbnail_map.insert(asset.path, thumbnail_path);
        }
    }
    
    // Build the asset tree recursively
    let assets_node = build_asset_node(&assets_path, &project_path, &thumbnail_map).await?;
    let mut total_files = 0;
    let mut total_directories = 0;
    count_nodes(&assets_node, &mut total_files, &mut total_directories);
    
    let asset_tree = ProjectAssetTree {
        project_name: project_name.to_string(),
        root_path: assets_path.to_string_lossy().to_string(),
        assets: vec![assets_node],
        generated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        total_files,
        total_directories,
    };
    
    // Cache the asset tree
    if let Some(cache) = redis_cache {
        let cache_lock = cache.lock().await;
        if cache_lock.cache_project_asset_tree(&asset_tree).await {
            info!("🌳 Successfully cached asset tree for project: {} ({} files, {} directories)", 
                  project_name, total_files, total_directories);
        } else {
            warn!("🌳 Failed to cache asset tree for project: {}", project_name);
        }
    }
    
    Ok(())
}

async fn build_asset_node(
    path: &std::path::Path, 
    project_root: &std::path::Path,
    thumbnail_map: &std::collections::HashMap<String, String>
) -> Result<CachedAssetNode, Box<dyn std::error::Error + Send + Sync>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let metadata = tokio_fs::metadata(path).await?;
    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    let relative_path = path.strip_prefix(project_root)?
        .to_string_lossy()
        .replace('\\', "/");
    
    let last_modified = metadata.modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    if path.is_dir() {
        // Build directory node with children
        let mut children = Vec::new();
        
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    let entry_name = entry_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    
                    // Skip hidden files and cache directories
                    if !entry_name.starts_with('.') {
                        if let Ok(child_node) = Box::pin(build_asset_node(&entry_path, project_root, thumbnail_map)).await {
                            children.push(child_node);
                        }
                    }
                }
            }
        }
        
        // Sort children: directories first, then files, both alphabetically
        children.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        // Always include the directory node, even if it's empty
        Ok(CachedAssetNode {
            name,
            path: relative_path,
            is_directory: true,
            file_size: None,
            last_modified: Some(last_modified),
            extension: None,
            file_type: Some("directory".to_string()),
            thumbnail_url: None,
            children: Some(children), // children will be empty Vec if no valid children found
        })
    } else {
        // Build file node
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase());
        
        let file_type = extension.as_ref()
            .map(|ext| get_file_type_from_extension(ext))
            .unwrap_or_else(|| "unknown".to_string());
        
        let thumbnail_url = thumbnail_map.get(&relative_path).cloned();
        
        Ok(CachedAssetNode {
            name,
            path: relative_path,
            is_directory: false,
            file_size: Some(metadata.len()),
            last_modified: Some(last_modified),
            extension,
            file_type: Some(file_type),
            thumbnail_url,
            children: None,
        })
    }
}

fn get_file_type_from_extension(ext: &str) -> String {
    match ext {
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tga" | "tiff" | "ico" | "svg" => "image".to_string(),
        "hdr" | "exr" => "hdr_image".to_string(),
        "glb" | "gltf" | "obj" | "fbx" | "dae" | "3ds" | "blend" | "stl" | "ply" => "model".to_string(),
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio".to_string(),
        "mp4" | "avi" | "mov" | "mkv" | "webm" | "wmv" => "video".to_string(),
        "js" | "ts" | "jsx" | "tsx" => "script".to_string(),
        "json" | "xml" | "yaml" | "yml" => "data".to_string(),
        "txt" | "md" | "rst" => "document".to_string(),
        "ren" => "renscript".to_string(),
        _ => "other".to_string(),
    }
}

fn count_nodes(node: &CachedAssetNode, files: &mut usize, directories: &mut usize) {
    if node.is_directory {
        *directories += 1;
        if let Some(children) = &node.children {
            for child in children {
                count_nodes(child, files, directories);
            }
        }
    } else {
        *files += 1;
    }
}


