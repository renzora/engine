//! Minimal sign-in for the splash screen.
//!
//! Talks to the same `renzora.com/api/auth/login` endpoint as `renzora_auth`
//! and writes `auth.json` to the same location so the editor's
//! `try_restore_session()` picks up the session automatically.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Mutex};

const API_BASE: &str = "https://renzora.com";

#[derive(Serialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub credit_balance: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserProfile,
}

#[derive(Serialize, Deserialize)]
struct StoredSession {
    access_token: String,
    refresh_token: String,
    user: UserProfile,
}

pub enum LoginOutcome {
    Success(UserProfile),
    Error(String),
}

#[derive(Resource, Default)]
pub struct SplashAuth {
    pub email: String,
    pub password: String,
    pub error: Option<String>,
    pub loading: bool,
    pub user: Option<UserProfile>,
    receiver: Option<Mutex<mpsc::Receiver<LoginOutcome>>>,
}

impl SplashAuth {
    pub fn new() -> Self {
        Self {
            user: load_saved_user(),
            ..Default::default()
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.user.is_some()
    }

    pub fn poll(&mut self) {
        let msg = self
            .receiver
            .as_ref()
            .and_then(|rx| rx.lock().ok())
            .and_then(|rx| rx.try_recv().ok());
        if let Some(msg) = msg {
            self.loading = false;
            match msg {
                LoginOutcome::Success(user) => {
                    self.user = Some(user);
                    self.password.clear();
                    self.error = None;
                }
                LoginOutcome::Error(e) => self.error = Some(e),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_login(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        let email = self.email.clone();
        let password = self.password.clone();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(Mutex::new(rx));
        std::thread::spawn(move || {
            let _ = tx.send(login_blocking(&email, &password));
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_login(&mut self) {
        self.error = Some("Sign-in is not supported in the browser yet".into());
    }

    pub fn sign_out(&mut self) {
        self.user = None;
        self.email.clear();
        self.password.clear();
        self.error = None;
        #[cfg(not(target_arch = "wasm32"))]
        delete_saved_session();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn login_blocking(email: &str, password: &str) -> LoginOutcome {
    let body = match serde_json::to_string(&LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    }) {
        Ok(s) => s,
        Err(e) => return LoginOutcome::Error(e.to_string()),
    };

    let response = match ureq::post(&format!("{API_BASE}/api/auth/login"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
    {
        Ok(r) => r,
        Err(e) => return LoginOutcome::Error(format!("Request failed: {e}")),
    };

    let text = match response.into_body().read_to_string() {
        Ok(s) => s,
        Err(e) => return LoginOutcome::Error(format!("Read failed: {e}")),
    };

    let resp: AuthResponse = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(_) => {
            return LoginOutcome::Error(
                "Invalid email or password".into(),
            );
        }
    };

    save_session(&resp);
    LoginOutcome::Success(resp.user)
}

#[cfg(not(target_arch = "wasm32"))]
fn auth_file() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|p| p.join("renzora").join("auth.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_session(resp: &AuthResponse) {
    let Some(path) = auth_file() else { return };
    let Some(parent) = path.parent() else { return };
    let _ = std::fs::create_dir_all(parent);
    let stored = StoredSession {
        access_token: resp.access_token.clone(),
        refresh_token: resp.refresh_token.clone(),
        user: resp.user.clone(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&stored) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_saved_user() -> Option<UserProfile> {
    let path = auth_file()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let stored: StoredSession = serde_json::from_str(&data).ok()?;
    Some(stored.user)
}

#[cfg(target_arch = "wasm32")]
fn load_saved_user() -> Option<UserProfile> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn delete_saved_session() {
    if let Some(path) = auth_file() {
        let _ = std::fs::remove_file(path);
    }
}
