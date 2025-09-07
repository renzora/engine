use actix_ws::{Session, MessageStream};
use futures_util::StreamExt;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;
use serde_json;
use base64::{Engine as _, engine::general_purpose};

use crate::types::{WsMessage, ClientConnection, ServerHealth, FileChange};
use crate::state::AppState;
use crate::config_manager::{get_current_server_config, set_base_path, set_projects_path, scan_for_engine_roots};

pub async fn handle_connection(
    mut session: Session,
    mut stream: MessageStream,
    state: AppState,
) {
    let client_id = Uuid::new_v4();
    let connected_at = Utc::now();
    
    info!("🔌 WebSocket connection established: {}", client_id);
    
    // Create client connection
    let client = ClientConnection {
        id: client_id,
        connected_at,
        watching_project: None,
        session: session.clone(),
    };
    
    // Add client to state
    state.add_client(client).await;
    
    // Send connection confirmation
    let welcome_msg = WsMessage::Connected {
        server_version: state.server_version.clone(),
        timestamp: connected_at,
    };
    
    let mut session_clone = session.clone();
    if let Err(e) = send_message(&mut session_clone, &welcome_msg).await {
        error!("Failed to send welcome message: {}", e);
        return;
    }
    
    // Start file change listener for this client
    let file_change_rx = state.subscribe_to_file_changes();
    let session_for_file_changes = session.clone();
    let file_change_task = tokio::spawn(async move {
        handle_file_changes(session_for_file_changes, file_change_rx).await;
    });
    
    // Handle incoming messages
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(actix_ws::Message::Text(text)) => {
                debug!("📨 Received text message: {}", text);
                
                match serde_json::from_str::<WsMessage>(&text.to_string()) {
                    Ok(ws_msg) => {
                        let mut session_mut = session.clone();
                        if let Err(e) = handle_ws_message(ws_msg, &mut session_mut, &state).await {
                            error!("Error handling message: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse WebSocket message: {}", e);
                        let error_msg = WsMessage::Error {
                            message: format!("Invalid message format: {}", e),
                            code: Some(400),
                        };
                        let mut session_err = session.clone();
                        let _ = send_message(&mut session_err, &error_msg).await;
                    }
                }
            }
            Ok(actix_ws::Message::Binary(data)) => {
                debug!("📨 Received binary message: {} bytes", data.len());
                // Handle binary messages if needed (large file uploads, etc.)
            }
            Ok(actix_ws::Message::Ping(data)) => {
                debug!("🏓 Received ping");
                if let Err(e) = session.pong(&data).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(actix_ws::Message::Pong(_)) => {
                debug!("🏓 Received pong");
            }
            Ok(actix_ws::Message::Close(reason)) => {
                info!("🔚 WebSocket closed: {:?}", reason);
                break;
            }
            Ok(actix_ws::Message::Continuation(_)) => {
                // Handle continuation frames if needed
                debug!("📨 Received continuation frame");
            }
            Ok(actix_ws::Message::Nop) => {
                // No-op message, ignore
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }
    
    // Cleanup
    file_change_task.abort();
    state.remove_client(&client_id).await;
    info!("🧹 Cleaned up client: {}", client_id);
}

async fn handle_ws_message(
    msg: WsMessage,
    session: &mut Session,
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    match msg {
        WsMessage::Ping => {
            send_message(session, &WsMessage::Pong).await?;
        }
        
        WsMessage::HealthCheck => {
            let uptime = (Utc::now() - state.startup_time).num_seconds() as u64;
            let client_count = state.get_client_count().await as u32;
            
            let health = ServerHealth {
                status: "healthy".to_string(),
                uptime_seconds: uptime,
                connections: client_count,
                memory_usage: get_memory_usage(),
                version: state.server_version.clone(),
            };
            
            let response = WsMessage::HealthCheckResponse { status: health };
            send_message(session, &response).await?;
        }
        
        WsMessage::FileRead { path, base_path: _ } => {
            match crate::file_sync::read_file_content(&path).await {
                Ok(content) => {
                    let response = WsMessage::FileReadResponse {
                        path,
                        content: Some(content),
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::FileReadResponse {
                        path,
                        content: None,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::FileBinaryRead { path, base_path: _ } => {
            match crate::file_sync::read_binary_file(&path).await {
                Ok(data) => {
                    let base64_content = general_purpose::STANDARD.encode(&data);
                    let response = WsMessage::FileBinaryReadResponse {
                        path,
                        content: Some(base64_content),
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::FileBinaryReadResponse {
                        path,
                        content: None,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::FileWrite { path, content, base_path: _ } => {
            let request = crate::types::WriteFileRequest { 
                content, 
                create_dirs: Some(true) 
            };
            match crate::file_sync::write_file_content(&path, &request).await {
                Ok(_) => {
                    let response = WsMessage::FileWriteResponse {
                        path,
                        success: true,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::FileWriteResponse {
                        path,
                        success: false,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::FileBinaryWrite { path, data, create_dirs, base_path: _ } => {
            let request = crate::types::WriteBinaryFileRequest { 
                data, 
                create_dirs 
            };
            match crate::file_sync::write_binary_file_content(&path, &request).await {
                Ok(_) => {
                    let response = WsMessage::FileBinaryWriteResponse {
                        path,
                        success: true,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::FileBinaryWriteResponse {
                        path,
                        success: false,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::FileDelete { path, base_path: _ } => {
            match crate::file_sync::delete_file_or_directory(&path).await {
                Ok(_) => {
                    let response = WsMessage::FileDeleteResponse {
                        path,
                        success: true,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::FileDeleteResponse {
                        path,
                        success: false,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::ListDirectory { path, base_path: _ } => {
            match crate::project_manager::list_directory_contents(&path).await {
                Ok(files) => {
                    let response = WsMessage::ListDirectoryResponse {
                        path,
                        items: files,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::ListDirectoryResponse {
                        path,
                        items: vec![],
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::ListProjects => {
            match crate::project_manager::list_projects().await {
                Ok(projects) => {
                    let response = WsMessage::ListProjectsResponse {
                        projects,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::ListProjectsResponse {
                        projects: vec![],
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::CreateProject { name, template } => {
            match crate::project_manager::create_project(&name, &template).await {
                Ok(project) => {
                    let response = WsMessage::CreateProjectResponse {
                        project: Some(project),
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::CreateProjectResponse {
                        project: None,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::GetConfig => {
            let config = get_current_server_config().await;
            let response = WsMessage::GetConfigResponse {
                config,
                error: None,
            };
            send_message(session, &response).await?;
        }
        
        WsMessage::SetBasePath { path } => {
            match set_base_path(&path).await {
                Ok(_) => {
                    let response = WsMessage::SetBasePathResponse {
                        success: true,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::SetBasePathResponse {
                        success: false,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::SetProjectsPath { path } => {
            match set_projects_path(&path).await {
                Ok(_) => {
                    let response = WsMessage::SetProjectsPathResponse {
                        success: true,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::SetProjectsPathResponse {
                        success: false,
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        WsMessage::ScanForEngineRoot => {
            match scan_for_engine_roots().await {
                Ok((found_paths, current_path)) => {
                    let response = WsMessage::ScanForEngineRootResponse {
                        found_paths,
                        current_path,
                        error: None,
                    };
                    send_message(session, &response).await?;
                }
                Err(e) => {
                    let response = WsMessage::ScanForEngineRootResponse {
                        found_paths: vec![],
                        current_path: String::new(),
                        error: Some(e.to_string()),
                    };
                    send_message(session, &response).await?;
                }
            }
        }
        
        // Add more message handlers as needed
        _ => {
            warn!("Unhandled message type: {:?}", std::mem::discriminant(&msg));
        }
    }
    
    Ok(())
}

async fn handle_file_changes(
    session: Session,
    mut file_change_rx: tokio::sync::broadcast::Receiver<FileChange>,
) {
    info!("📡 Starting file change listener for client");
    
    while let Ok(change) = file_change_rx.recv().await {
        let message = WsMessage::FileChanges {
            changes: vec![change],
        };
        
        let mut session_clone = session.clone();
        if let Err(e) = send_message(&mut session_clone, &message).await {
            error!("Failed to send file change notification: {}", e);
            break;
        }
    }
    
    info!("📡 File change listener stopped");
}

async fn send_message(session: &mut Session, message: &WsMessage) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(message)?;
    session.text(json).await?;
    Ok(())
}

fn get_memory_usage() -> u64 {
    use sysinfo::System;
    let mut system = System::new_all();
    system.refresh_memory();
    system.used_memory()
}