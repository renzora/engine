use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use log::{info, warn, error, debug};
use tokio::sync::broadcast;
use crate::file_watcher::get_file_change_receiver;

pub type WsSink = SplitSink<WebSocketStream<TcpStream>, Message>;

pub async fn start_websocket_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(&addr).await?;
    info!("🔌 WebSocket server listening on: ws://{}", addr);

    while let Ok((stream, client_addr)) = listener.accept().await {
        info!("📡 New WebSocket connection from: {}", client_addr);
        tokio::spawn(handle_websocket_connection(stream, client_addr));
    }

    Ok(())
}

async fn handle_websocket_connection(stream: TcpStream, client_addr: SocketAddr) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("❌ WebSocket handshake failed for {}: {}", client_addr, e);
            return;
        }
    };

    info!("✅ WebSocket connection established with {}", client_addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Send initial connection message
    let welcome_msg = serde_json::json!({
        "type": "connected",
        "message": "WebSocket connection established"
    });
    
    if let Ok(welcome_text) = serde_json::to_string(&welcome_msg) {
        if let Err(e) = ws_sender.send(Message::Text(welcome_text)).await {
            error!("❌ Failed to send welcome message to {}: {}", client_addr, e);
            return;
        }
    }

    // Get file change receiver
    let mut file_change_receiver = match get_file_change_receiver() {
        Some(receiver) => receiver,
        None => {
            error!("❌ No file change receiver available for {}", client_addr);
            return;
        }
    };

    info!("📡 WebSocket client {} subscribed to file changes", client_addr);

    loop {
        tokio::select! {
            // Handle incoming WebSocket messages from client
            ws_msg = ws_receiver.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(_text))) => {
                        // Client sent a text message - we can ignore for now
                        debug!("📡 Received message from {}", client_addr);
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("📡 WebSocket connection closed by client {}", client_addr);
                        break;
                    }
                    Some(Err(e)) => {
                        error!("❌ WebSocket error from {}: {}", client_addr, e);
                        break;
                    }
                    None => {
                        info!("📡 WebSocket stream ended for {}", client_addr);
                        break;
                    }
                    _ => {}
                }
            }

            // Handle file change events
            file_change = file_change_receiver.recv() => {
                match file_change {
                    Ok(change_message) => {
                        debug!("📡 Broadcasting file change to {}: {}", client_addr, change_message);
                        
                        let ws_message = serde_json::json!({
                            "type": "file_change",
                            "message": change_message
                        });
                        
                        if let Ok(message_text) = serde_json::to_string(&ws_message) {
                            if let Err(e) = ws_sender.send(Message::Text(message_text)).await {
                                error!("❌ Failed to send file change to {}: {}", client_addr, e);
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("📡 WebSocket client {} lagged {} messages", client_addr, count);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("📡 File change broadcaster closed, ending WebSocket connection for {}", client_addr);
                        break;
                    }
                }
            }
        }
    }

    info!("📡 WebSocket connection with {} ended", client_addr);
}