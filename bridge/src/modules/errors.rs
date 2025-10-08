use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("File operation failed: {0}")]
    FileOperation(#[from] std::io::Error),
    
    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Cache operation failed: {0}")]
    Cache(String),
    
    #[error("HTTP request failed: {0}")]
    Http(String),
    
    #[error("Model processing failed: {0}")]
    ModelProcessing(String),
    
    #[error("Project operation failed: {0}")]
    Project(String),
    
    #[error("Validation failed: {0}")]
    Validation(String),
    
    #[error("Encoding/Decoding failed: {0}")]
    Encoding(String),
    
    #[error("System error: {0}")]
    System(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<base64::DecodeError> for BridgeError {
    fn from(err: base64::DecodeError) -> Self {
        BridgeError::Encoding(format!("Base64 decode error: {}", err))
    }
}

impl From<String> for BridgeError {
    fn from(err: String) -> Self {
        BridgeError::Unknown(err)
    }
}

impl From<&str> for BridgeError {
    fn from(err: &str) -> Self {
        BridgeError::Unknown(err.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for BridgeError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        BridgeError::Unknown(err.to_string())
    }
}

pub type BridgeResult<T> = Result<T, BridgeError>;