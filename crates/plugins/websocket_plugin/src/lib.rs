use editor_plugin_api::prelude::*;
use editor_plugin_api::ffi::FfiTransform;
use std::thread;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use crossbeam_channel::{unbounded, Receiver, Sender};
use tungstenite::{accept, Message};
use serde::{Serialize, Deserialize};

/// Commands that can be sent to the engine via WebSocket
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum RemoteCommand {
    #[serde(rename = "spawn")]
    Spawn {
        name: String,
        position: [f32; 3],
    },
}

/// Unique ID for connected clients
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ClientId(pub u64);

/// Mirror of editor_plugin_api::UiId for serialization
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SerializableUiId(pub u64);

/// Mirror of editor_plugin_api::TextStyle for serialization
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

/// Mirror of editor_plugin_api::Align for serialization
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SerializableAlign {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Mirror of editor_plugin_api::Widget for serialization
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
    Separator,
    TextInput {
        value: String,
        placeholder: String,
        id: SerializableUiId,
    },
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
    ScrollArea {
        child: Box<SerializableWidget>,
        max_height: Option<f32>,
        horizontal: bool,
    },
}

/// Mirror of engine's UI event structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum SerializableUiEvent {
    #[serde(rename = "button_clicked")]
    ButtonClicked { id: u64 },
    #[serde(rename = "text_input_changed")]
    TextInputChanged { id: u64, value: String },
    #[serde(rename = "text_input_submitted")]
    TextInputSubmitted { id: u64, value: String },
    #[serde(rename = "checkbox_toggled")]
    CheckboxToggled { id: u64, checked: bool },
    #[serde(rename = "slider_changed")]
    SliderChanged { id: u64, value: f32 },
}

/// Messages sent from the Server threads to the plugin
enum PluginMsg {
    ClientConnected(ClientId, String),
    ClientDisconnected(ClientId),
    MessageReceived(ClientId, String),
    Error(String),
    Started(String),
}

/// Commands sent from the plugin to the server
enum ServerCmd {
    Broadcast(String),
    Shutdown,
}

pub struct WebSocketPlugin {
    port: String,
    status: String,
    running: bool,
    clients: HashMap<ClientId, String>,
    messages: Vec<String>,
    broadcast_text: String,
    
    // Threading
    server_tx: Option<Sender<ServerCmd>>,
    plugin_rx: Option<Receiver<PluginMsg>>,
}

impl WebSocketPlugin {
    pub fn new() -> Self {
        Self {
            port: "8080".into(),
            status: "Stopped".into(),
            running: false,
            clients: HashMap::new(),
            messages: Vec::new(),
            broadcast_text: "".into(),
            server_tx: None,
            plugin_rx: None,
        }
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.renzora.websocket-server", "WebSocket Server", "1.0.0")
            .author("Renzora Dev")
            .description("A plugin that acts as a WebSocket server")
            .capability(PluginCapability::Panel)
    }

    fn start_server(&mut self) {
        if self.running {
            return;
        }

        let addr = format!("127.0.0.1:{}", self.port);
        let (p_tx, p_rx) = unbounded::<PluginMsg>();
        let (s_tx, s_rx) = unbounded::<ServerCmd>();

        self.plugin_rx = Some(p_rx);
        self.server_tx = Some(s_tx);
        self.running = true;
        self.status = format!("Starting on {}...", addr);

        let p_tx_main = p_tx.clone();
        let addr_clone = addr.clone();

        thread::spawn(move || {
            let listener = match TcpListener::bind(&addr_clone) {
                Ok(l) => l,
                Err(e) => {
                    let _ = p_tx_main.send(PluginMsg::Error(format!("Bind error: {}", e)));
                    return;
                }
            };
            
            listener.set_nonblocking(true).expect("Cannot set non-blocking");
            let _ = p_tx_main.send(PluginMsg::Started(addr_clone));

            let mut clients: HashMap<ClientId, Sender<String>> = HashMap::new();
            let mut next_id = 1;

            loop {
                // 1. Check for commands from the plugin
                while let Ok(cmd) = s_rx.try_recv() {
                    match cmd {
                        ServerCmd::Broadcast(text) => {
                            let mut disconnected = Vec::new();
                            for (id, tx) in &clients {
                                if let Err(_) = tx.send(text.clone()) {
                                    disconnected.push(*id);
                                }
                            }
                            for id in disconnected {
                                clients.remove(&id);
                                let _ = p_tx_main.send(PluginMsg::ClientDisconnected(id));
                            }
                        }
                        ServerCmd::Shutdown => return,
                    }
                }

                // 2. Accept new connections
                match listener.accept() {
                    Ok((stream, addr)) => {
                        let id = ClientId(next_id);
                        next_id += 1;
                        let addr_str = addr.to_string();
                        let _ = p_tx_main.send(PluginMsg::ClientConnected(id, addr_str.clone()));

                        let (c_tx, c_rx) = unbounded::<String>();
                        clients.insert(id, c_tx);

                        let p_tx_client = p_tx_main.clone();
                        thread::spawn(move || {
                            handle_client(id, stream, c_rx, p_tx_client);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        let _ = p_tx_main.send(PluginMsg::Error(format!("Accept error: {}", e)));
                    }
                }
            }
        });
    }

    fn stop_server(&mut self) {
        if let Some(tx) = self.server_tx.take() {
            let _ = tx.send(ServerCmd::Shutdown);
        }
        self.running = false;
        self.status = "Stopped".into();
        self.clients.clear();
        self.plugin_rx = None;
    }
}

fn handle_client(id: ClientId, stream: TcpStream, cmd_rx: Receiver<String>, p_tx: Sender<PluginMsg>) {
    let mut websocket = match accept(stream) {
        Ok(ws) => ws,
        Err(e) => {
            let _ = p_tx.send(PluginMsg::Error(format!("Handshake failed: {}", e)));
            return;
        }
    };

    websocket.get_mut().set_nonblocking(true).unwrap();

    loop {
        // Send messages from server to client
        while let Ok(text) = cmd_rx.try_recv() {
            if let Err(_) = websocket.send(Message::Text(text.into())) {
                let _ = p_tx.send(PluginMsg::ClientDisconnected(id));
                return;
            }
        }

        // Receive messages from client
        match websocket.read() {
            Ok(Message::Text(text)) => {
                let _ = p_tx.send(PluginMsg::MessageReceived(id, text));
            }
            Ok(Message::Binary(_)) => {
                let _ = p_tx.send(PluginMsg::MessageReceived(id, "<Binary Data>".into()));
            }
            Ok(Message::Close(_)) => {
                let _ = p_tx.send(PluginMsg::ClientDisconnected(id));
                return;
            }
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data, wait
            }
            Err(_) => {
                let _ = p_tx.send(PluginMsg::ClientDisconnected(id));
                return;
            }
            _ => {}
        }

        thread::sleep(std::time::Duration::from_millis(1));
    }
}

impl WebSocketPlugin {
    pub fn on_load(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
        api.register_panel(
            "ws_server",
            "WS Server",
            FfiPanelLocation::Bottom,
            Some(egui_phosphor::regular::BROADCAST),
            [250.0, 150.0]
        );
        // CRITICAL: Subscribe to UI events
        api.subscribe("ui.*");
        api.log_info("WebSocket Server Plugin loaded");
        Ok(())
    }

    pub fn on_unload(&mut self, _api: &FfiEditorApi) {
        self.stop_server();
    }

    pub fn on_update(&mut self, api: &FfiEditorApi, _dt: f32) {
        if let Some(rx) = &self.plugin_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    PluginMsg::Started(addr) => {
                        self.status = format!("Running on {}", addr);
                        api.log_info(&format!("[WS Server] Started on {}", addr));
                    }
                    PluginMsg::Error(e) => {
                        self.running = false;
                        self.status = format!("Error: {}", e);
                        api.log_error(&format!("[WS Server] {}", e));
                    }
                    PluginMsg::ClientConnected(id, addr) => {
                        self.clients.insert(id, addr.clone());
                        api.log_info(&format!("[WS Server] Client connected: {} ({})", id.0, addr));
                    }
                    PluginMsg::ClientDisconnected(id) => {
                        self.clients.remove(&id);
                        api.log_info(&format!("[WS Server] Client disconnected: {}", id.0));
                    }
                    PluginMsg::MessageReceived(id, text) => {
                        let name = self.clients.get(&id).cloned().unwrap_or_else(|| "Unknown".into());
                        self.messages.push(format!("[Client {}] {}", id.0, text));
                        if self.messages.len() > 100 { self.messages.remove(0); }
                        api.log_info(&format!("[WS Server] Received from {}: {}", name, text));

                        // NEW: Handle remote commands
                        if let Ok(cmd) = serde_json::from_str::<RemoteCommand>(&text) {
                            match cmd {
                                RemoteCommand::Spawn { name, position } => {
                                    let transform = FfiTransform {
                                        translation: position,
                                        ..Default::default()
                                    };
                                    let mut def = EntityDefinition::new(&name);
                                    def.node_type = "Cube".to_string();
                                    def.transform = transform;
                                    api.spawn_entity(&def);
                                    api.log_info(&format!("[WS Server] Executed: Spawn {} at {:?}", name, position));
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut widgets = vec![
            SerializableWidget::Label {
                text: "WebSocket Server".into(),
                style: SerializableTextStyle::Heading2,
            },
            SerializableWidget::Separator,
        ];

        // Config Row
        widgets.push(SerializableWidget::Row {
            spacing: 8.0,
            align: SerializableAlign::Center,
            children: vec![
                SerializableWidget::Label { text: "Port:".into(), style: SerializableTextStyle::Label },
                SerializableWidget::TextInput {
                    value: self.port.clone(),
                    placeholder: "8080".into(),
                    id: SerializableUiId(1),
                },
                if self.running {
                    SerializableWidget::Button { label: "Stop Server".into(), id: SerializableUiId(2), enabled: true }
                } else {
                    SerializableWidget::Button { label: "Start Server".into(), id: SerializableUiId(3), enabled: true }
                },
            ],
        });

        widgets.push(SerializableWidget::Label {
            text: format!("Status: {} | Connected Clients: {}", self.status, self.clients.len()),
            style: SerializableTextStyle::Body,
        });
        widgets.push(SerializableWidget::Separator);

        // Broadcast Row
        if self.running {
            widgets.push(SerializableWidget::Row {
                spacing: 8.0,
                align: SerializableAlign::Center,
                children: vec![
                    SerializableWidget::TextInput {
                        value: self.broadcast_text.clone(),
                        placeholder: "Broadcast message to all clients...".into(),
                        id: SerializableUiId(4),
                    },
                    SerializableWidget::Button {
                        label: "Broadcast".into(),
                        id: SerializableUiId(5),
                        enabled: !self.clients.is_empty(),
                    },
                ],
            });
            widgets.push(SerializableWidget::Separator);
        }

        // Log
        widgets.push(SerializableWidget::Label {
            text: "Server Log".into(),
            style: SerializableTextStyle::Heading3,
        });

        let mut log_widgets = Vec::new();
        if self.messages.is_empty() {
            log_widgets.push(SerializableWidget::Label { text: "No traffic yet.".into(), style: SerializableTextStyle::Caption });
        } else {
            for msg in self.messages.iter().rev() {
                log_widgets.push(SerializableWidget::Label { text: msg.clone(), style: SerializableTextStyle::Code });
            }
        }

        widgets.push(SerializableWidget::ScrollArea {
            child: Box::new(SerializableWidget::Column {
                children: log_widgets,
                spacing: 4.0,
                align: SerializableAlign::Start,
            }),
            max_height: Some(200.0),
            horizontal: false,
        });

        if let Ok(json) = serde_json::to_string(&widgets) {
            api.set_panel_content_json("ws_server", &json);
        }

        api.set_status_item(
            "ws_server_status",
            &format!("WS Server: {}", if self.running { "Online" } else { "Offline" }),
            Some(if self.running { egui_phosphor::regular::GLOBE } else { egui_phosphor::regular::PLUGS }),
            Some(&format!("Clients: {}", self.clients.len())),
            true,
            0
        );
    }

    pub fn on_load_ffi(&mut self, api: &FfiEditorApi) -> Result<(), PluginError> {
        self.on_load(api)
    }
    pub fn on_unload_ffi(&mut self, api: &FfiEditorApi) {
        self.on_unload(api)
    }
    pub fn on_update_ffi(&mut self, api: &FfiEditorApi, dt: f32) {
        self.on_update(api, dt)
    }
    pub fn on_event_ffi(&mut self, _api: &FfiEditorApi, event_json: &str) {
        if let Ok(event) = serde_json::from_str::<SerializableUiEvent>(event_json) {
            match event {
                SerializableUiEvent::TextInputChanged { id, value } if id == 1 => self.port = value,
                SerializableUiEvent::ButtonClicked { id } if id == 2 => self.stop_server(),
                SerializableUiEvent::ButtonClicked { id } if id == 3 => self.start_server(),
                SerializableUiEvent::TextInputChanged { id, value } if id == 4 => self.broadcast_text = value,
                SerializableUiEvent::ButtonClicked { id } if id == 5 => {
                    if let Some(tx) = &self.server_tx {
                        let _ = tx.send(ServerCmd::Broadcast(self.broadcast_text.clone()));
                        self.broadcast_text = "".into();
                    }
                }
                SerializableUiEvent::TextInputSubmitted { id, value } if id == 4 => {
                     if let Some(tx) = &self.server_tx {
                        let _ = tx.send(ServerCmd::Broadcast(value));
                        self.broadcast_text = "".into();
                    }
                }
                _ => {}
            }
        }
    }
}

declare_plugin!(WebSocketPlugin, WebSocketPlugin::new());
