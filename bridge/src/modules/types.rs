use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct FileChangeEvent {
    pub event_type: String,
    pub paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub files: Vec<FileInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
}

#[derive(Deserialize)]
pub struct WriteFileRequest {
    pub content: String,
}

#[derive(Deserialize)]
pub struct WriteBinaryFileRequest {
    pub base64_content: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub template: Option<String>,
    pub settings: Option<serde_json::Value>,
}


#[derive(Serialize)]
pub struct ApiResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}