use editor_plugin_api::prelude::*;
use std::thread;
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::{Serialize, Deserialize};
use tiny_http::{Server, Response, Method};

// --- MCP / JSON-RPC Types ---

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: serde_json::Value,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// --- Plugin Internal Communication ---

enum ServerMsg {
    Request {
        id: serde_json::Value,
        method: String,
        params: Option<serde_json::Value>,
        response_tx: Sender<serde_json::Value>,
    },
    Error(String),
}

// --- Serializable UI Types (Mirrors exactly from engine's Widget enum for FFI) ---

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SerializableUiId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SerializableTextStyle {
    #[default]
    Body,
    Heading1,
    Heading2,
    Heading3,
    Caption,
    Code,
    Label,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SerializableAlign {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SerializableWidget {
    Label {
        text: String,
        style: SerializableTextStyle,
    },
    Button {
        label: String,
        id: SerializableUiId,
        enabled: bool,
    },
    IconButton {
        icon: String,
        tooltip: String,
        id: SerializableUiId,
        enabled: bool,
    },
    Separator,
    Row {
        children: Vec<SerializableWidget>,
        spacing: f32,
        align: SerializableAlign,
    },
    Column {
        children: Vec<SerializableWidget>,
        spacing: f32,
        align: SerializableAlign,
    },
    Spacer {
        size: SerializableSize,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SerializableSize {
    Auto,
    Fixed(f32),
    Percent(f32),
    Fill,
    FillPortion(f32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum SerializableUiEvent {
    #[serde(rename = "button_clicked")]
    ButtonClicked { id: u64 },
}

// --- Plugin Implementation ---

pub struct McpServerPlugin {
    port: u16,
    running: bool,
    status: String,
    requests_received: u64,
    last_request_method: String,
    
    // Communication
    server_rx: Option<Receiver<ServerMsg>>,
    shutdown_tx: Option<Sender<()>>,
}

impl McpServerPlugin {
    pub fn new() -> Self {
        Self {
            port: 3000,
            running: false,
            status: "Stopped".into(),
            requests_received: 0,
            last_request_method: "None".into(),
            server_rx: None,
            shutdown_tx: None,
        }
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.renzora.mcp-server", "MCP Server", "0.1.0")
            .author("Renzora Dev")
            .description("Model Context Protocol (MCP) server for remote engine control")
            .capability(PluginCapability::Panel)
    }

    fn start_server(&mut self, api: &FfiEditorApi) {
        if self.running { return; }

        let (msg_tx, msg_rx) = unbounded::<ServerMsg>();
        let (stop_tx, stop_rx) = unbounded::<()>();
        let port = self.port;
        
        self.server_rx = Some(msg_rx);
        self.shutdown_tx = Some(stop_tx);
        self.running = true;
        self.status = format!("Running on port {}", port);

        let msg_tx_clone = msg_tx.clone();
        
        thread::spawn(move || {
            let server = match Server::http(format!("127.0.0.1:{}", port)) {
                Ok(s) => s,
                Err(e) => {
                    let _ = msg_tx_clone.send(ServerMsg::Error(format!("Failed to bind to port {}: {}", port, e)));
                    return;
                }
            };

            loop {
                // Check for shutdown
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                // Wait for a request with timeout so we can check shutdown regularly
                if let Ok(Some(mut request)) = server.recv_timeout(std::time::Duration::from_millis(100)) {
                    if request.method() != &Method::Post {
                        let _ = request.respond(Response::from_string("Only POST /rpc is supported").with_status_code(405));
                        continue;
                    }

                    let mut content = String::new();
                    if let Err(e) = request.as_reader().read_to_string(&mut content) {
                        let _ = request.respond(Response::from_string(format!("Read error: {}", e)).with_status_code(400));
                        continue;
                    }

                    match serde_json::from_str::<JsonRpcRequest>(&content) {
                        Ok(rpc_req) => {
                            let (resp_tx, resp_rx) = unbounded::<serde_json::Value>();
                            
                            // Send to main thread for processing
                            if let Err(_) = msg_tx_clone.send(ServerMsg::Request {
                                id: rpc_req.id.clone(),
                                method: rpc_req.method,
                                params: rpc_req.params,
                                response_tx: resp_tx,
                            }) {
                                let _ = request.respond(Response::from_string("Plugin shutting down").with_status_code(503));
                                break;
                            }

                            // Wait for response from main thread
                            match resp_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                                Ok(result) => {
                                    let resp = JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: rpc_req.id,
                                        result: Some(result),
                                        error: None,
                                    };
                                    let json = serde_json::to_string(&resp).unwrap_or_default();
                                    let _ = request.respond(Response::from_string(json).with_header(
                                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
                                    ));
                                }
                                Err(_) => {
                                    let resp = JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: rpc_req.id,
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32000,
                                            message: "Request timed out on editor side".to_string(),
                                        }),
                                    };
                                    let json = serde_json::to_string(&resp).unwrap_or_default();
                                    let _ = request.respond(Response::from_string(json).with_status_code(500));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = request.respond(Response::from_string(format!("Invalid JSON-RPC: {}", e)).with_status_code(400));
                        }
                    }
                }
            }
        });

        api.log_info(&format!("MCP Server started on port {}", port));
    }

    fn stop_server(&mut self, api: &FfiEditorApi) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.running = false;
        self.status = "Stopped".into();
        self.server_rx = None;
        api.log_info("MCP Server stopped");
    }

    fn handle_mcp_request(&mut self, api: &FfiEditorApi, method: &str, params: Option<serde_json::Value>) -> serde_json::Value {
        self.requests_received += 1;
        self.last_request_method = method.to_string();

        match method {
            "initialize" => {
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "resources": {},
                        "tools": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "renzora-engine-mcp",
                        "version": "0.1.0"
                    }
                })
            }
            "listResources" => {
                serde_json::json!({
                    "resources": [
                        {
                            "uri": "scene://selection",
                            "name": "Current Selection",
                            "description": "Details of the currently selected entity",
                            "mimeType": "application/json"
                        }
                    ]
                })
            }
            "listTools" => {
                serde_json::json!({
                    "tools": [
                        {
                            "name": "spawn_entity",
                            "description": "Spawn a new entity in the scene",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "position": { 
                                        "type": "array", 
                                        "items": { "type": "number" },
                                        "minItems": 3, "maxItems": 3 
                                    }
                                },
                                "required": ["name"]
                            }
                        },
                        {
                            "name": "set_transform",
                            "description": "Set the transform of an entity",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "entity_id": { "type": "integer" },
                                    "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                                    "rotation": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4 },
                                    "scale": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }
                                },
                                "required": ["entity_id"]
                            }
                        },
                        {
                            "name": "despawn_entity",
                            "description": "Remove an entity from the scene",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "entity_id": { "type": "integer" }
                                },
                                "required": ["entity_id"]
                            }
                        },
                        {
                            "name": "log_message",
                            "description": "Log a message to the Renzora console",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "message": { "type": "string" },
                                    "level": { "type": "string", "enum": ["info", "warn", "error"] }
                                },
                                "required": ["message"]
                            }
                        }
                    ]
                })
            }
            "callTool" => {
                self.handle_call_tool(api, params)
            }
            "readResource" => {
                self.handle_read_resource(api, params)
            }
            _ => {
                serde_json::json!({
                    "error": {
                        "code": -32601,
                        "message": format!("Method '{}' not found", method)
                    }
                })
            }
        }
    }

    fn handle_call_tool(&mut self, api: &FfiEditorApi, params: Option<serde_json::Value>) -> serde_json::Value {
        let params = params.unwrap_or_default();
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or_default();

        match name {
            "spawn_entity" => {
                let entity_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("New Entity");
                let pos = args.get("position").and_then(|v| v.as_array());
                
                let mut transform = FfiTransform::default();
                if let Some(p) = pos {
                    if p.len() == 3 {
                        transform.translation = [
                            p[0].as_f64().unwrap_or(0.0) as f32,
                            p[1].as_f64().unwrap_or(0.0) as f32,
                            p[2].as_f64().unwrap_or(0.0) as f32,
                        ];
                    }
                }
                
                let id = api.spawn_entity(entity_name, &transform);
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Spawned entity '{}' with ID {}", entity_name, id.0)
                    }]
                })
            }
            "set_transform" => {
                let id_val = args.get("entity_id").and_then(|v| v.as_u64());
                if let Some(id_u64) = id_val {
                    let entity = FfiEntityId(id_u64);
                    let mut transform = api.get_entity_transform(entity);
                    
                    if let Some(p) = args.get("position").and_then(|v| v.as_array()) {
                        if p.len() == 3 {
                            transform.translation = [
                                p[0].as_f64().unwrap_or(0.0) as f32,
                                p[1].as_f64().unwrap_or(0.0) as f32,
                                p[2].as_f64().unwrap_or(0.0) as f32,
                            ];
                        }
                    }
                    if let Some(r) = args.get("rotation").and_then(|v| v.as_array()) {
                        if r.len() == 4 {
                            transform.rotation = [
                                r[0].as_f64().unwrap_or(0.0) as f32,
                                r[1].as_f64().unwrap_or(0.0) as f32,
                                r[2].as_f64().unwrap_or(0.0) as f32,
                                r[3].as_f64().unwrap_or(1.0) as f32,
                            ];
                        }
                    }
                    if let Some(s) = args.get("scale").and_then(|v| v.as_array()) {
                        if s.len() == 3 {
                            transform.scale = [
                                s[0].as_f64().unwrap_or(1.0) as f32,
                                s[1].as_f64().unwrap_or(1.0) as f32,
                                s[2].as_f64().unwrap_or(1.0) as f32,
                            ];
                        }
                    }
                    api.set_entity_transform(entity, &transform);
                    serde_json::json!({
                        "content": [{"type": "text", "text": format!("Updated transform for entity {}", id_u64)}]
                    })
                } else {
                    serde_json::json!({
                        "isError": true,
                        "content": [{"type": "text", "text": "Missing entity_id"}]
                    })
                }
            }
            "despawn_entity" => {
                let id_val = args.get("entity_id").and_then(|v| v.as_u64());
                if let Some(id_u64) = id_val {
                    api.despawn_entity(FfiEntityId(id_u64));
                    serde_json::json!({
                        "content": [{"type": "text", "text": format!("Despawned entity {}", id_u64)}]
                    })
                } else {
                    serde_json::json!({ "isError": true, "content": [{"type": "text", "text": "Missing entity_id"}] })
                }
            }
            "log_message" => {
                let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let level = args.get("level").and_then(|v| v.as_str()).unwrap_or("info");
                match level {
                    "warn" => api.log_warn(msg),
                    "error" => api.log_error(msg),
                    _ => api.log_info(msg),
                }
                serde_json::json!({ "content": [{"type": "text", "text": "Logged message"}] })
            }
            _ => {
                serde_json::json!({
                    "isError": true,
                    "content": [{"type": "text", "text": format!("Tool '{}' not found", name)}]
                })
            }
        }
    }

    fn handle_read_resource(&mut self, api: &FfiEditorApi, params: Option<serde_json::Value>) -> serde_json::Value {
        let uri = params.as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        match uri {
            "scene://selection" => {
                let id = api.get_selected_entity();
                if id.is_valid() {
                    let name = api.get_entity_name(id);
                    let transform = api.get_entity_transform(id);
                    serde_json::json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string(&serde_json::json!({
                                "id": id.0,
                                "name": name,
                                "transform": {
                                    "translation": transform.translation,
                                    "rotation": transform.rotation,
                                    "scale": transform.scale
                                }
                            })).unwrap_or_default()
                        }]
                    })
                } else {
                    serde_json::json!({ "contents": [{ "uri": uri, "text": "No entity selected" }] })
                }
            }
            _ => {
                serde_json::json!({
                    "isError": true,
                    "content": [{"type": "text", "text": format!("Resource '{}' not found", uri)}]
                })
            }
        }
    }
}

// --- FFI Entry Points ---

impl McpServerPlugin {
    pub fn on_load_ffi(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
        api.register_panel(
            "mcp_server",
            "MCP Server",
            FfiPanelLocation::Bottom,
            Some(egui_phosphor::regular::ROBOT),
            [250.0, 150.0]
        );
        api.subscribe("ui.*");
        api.log_info("MCP Server Plugin loaded");
        self.start_server(api);
        Ok(())
    }

    pub fn on_unload_ffi(&mut self, api: &FfiEditorApi) {
        self.stop_server(api);
    }

    pub fn on_update_ffi(&mut self, api: &FfiEditorApi, _dt: f32) {
        if let Some(rx) = self.server_rx.take() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ServerMsg::Request { id: _, method, params, response_tx } => {
                        let result = self.handle_mcp_request(api, &method, params);
                        let _ = response_tx.send(result);
                    }
                    ServerMsg::Error(e) => {
                        api.log_error(&format!("[MCP Error] {}", e));
                        self.status = format!("Error: {}", e);
                    }
                }
            }
            self.server_rx = Some(rx);
        }

        // Use exact Widget enum structure from engine
        let widgets = vec![
            SerializableWidget::Label {
                text: "MCP Server Status".into(),
                style: SerializableTextStyle::Heading2,
            },
            SerializableWidget::Separator,
            SerializableWidget::Row {
                spacing: 8.0,
                align: SerializableAlign::Center,
                children: vec![
                    SerializableWidget::Label { text: format!("Port: {}", self.port), style: SerializableTextStyle::Label },
                    if self.running {
                        SerializableWidget::Button { label: "Stop Server".into(), id: SerializableUiId(1), enabled: true }
                    } else {
                        SerializableWidget::Button { label: "Start Server".into(), id: SerializableUiId(2), enabled: true }
                    },
                ],
            },
            SerializableWidget::Label {
                text: format!("Status: {}", self.status),
                style: SerializableTextStyle::Body,
            },
            SerializableWidget::Label {
                text: format!("Requests: {} | Last: {}", self.requests_received, self.last_request_method),
                style: SerializableTextStyle::Caption,
            },
            SerializableWidget::Separator,
            SerializableWidget::Label { text: "Exposed Tools:".into(), style: SerializableTextStyle::Label },
            SerializableWidget::Label { text: "• spawn_entity, set_transform, despawn_entity, log_message".into(), style: SerializableTextStyle::Caption },
        ];

        if let Ok(json) = serde_json::to_string(&widgets) {
            api.set_panel_content_json("mcp_server", &json);
        }

        api.set_status_item(
            "mcp_server_status",
            &format!("MCP Server: {}", if self.running { "Online" } else { "Offline" }),
            Some(egui_phosphor::regular::ROBOT),
            Some(&format!("Port: {}", self.port)),
            true,
            0
        );
    }

    pub fn on_event_ffi(&mut self, api: &FfiEditorApi, event_json: &str) {
        if let Ok(event) = serde_json::from_str::<SerializableUiEvent>(event_json) {
            match event {
                SerializableUiEvent::ButtonClicked { id } if id == 1 => self.stop_server(api),
                SerializableUiEvent::ButtonClicked { id } if id == 2 => self.start_server(api),
                _ => {}
            }
        }
    }
}

declare_plugin!(McpServerPlugin, McpServerPlugin::new());
