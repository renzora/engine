use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use log::{info, warn, error, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::io::Write;
use reqwest;

pub struct EmbeddedRedisServer {
    process: Option<Child>,
    handle: Option<JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
    port: u16,
    redis_path: PathBuf,
}

impl EmbeddedRedisServer {
    pub fn new(port: Option<u16>) -> Self {
        let bridge_dir = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
            
        Self {
            process: None,
            handle: None,
            is_running: Arc::new(AtomicBool::new(false)),
            port: port.unwrap_or(6379),
            redis_path: bridge_dir.join("redis-server.exe"),
        }
    }

    async fn ensure_redis_binary(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.redis_path.exists() {
            debug!("🔴 Redis binary already exists at {:?}", self.redis_path);
            return Ok(());
        }

        info!("🔴 Downloading Redis server binary...");
        
        // Download Redis for Windows from official source
        let redis_url = "https://github.com/microsoftarchive/redis/releases/download/win-3.0.504/Redis-x64-3.0.504.zip";
        
        let response = reqwest::get(redis_url).await?;
        let bytes = response.bytes().await?;
        
        // Create temporary zip file
        let temp_zip = self.redis_path.parent().unwrap().join("redis.zip");
        let mut file = std::fs::File::create(&temp_zip)?;
        file.write_all(&bytes)?;
        drop(file);
        
        // Extract redis-server.exe from zip
        self.extract_redis_binary(&temp_zip).await?;
        
        // Clean up
        let _ = std::fs::remove_file(temp_zip);
        
        info!("🔴 Redis binary downloaded successfully");
        Ok(())
    }
    
    async fn extract_redis_binary(&self, zip_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Read;
        
        let file = std::fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        // Find redis-server.exe in the archive
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name().ends_with("redis-server.exe") {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)?;
                
                let mut output_file = std::fs::File::create(&self.redis_path)?;
                output_file.write_all(&contents)?;
                
                debug!("🔴 Extracted redis-server.exe to {:?}", self.redis_path);
                return Ok(());
            }
        }
        
        Err("redis-server.exe not found in archive".into())
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Ensure Redis binary exists
        if let Err(e) = self.ensure_redis_binary().await {
            warn!("🔴 Failed to ensure Redis binary: {}, falling back to system Redis", e);
            return self.start_system_redis().await;
        }

        info!("🔴 Starting embedded Redis server on port {}", self.port);

        // Create Redis config
        let config_content = format!(
            r#"port {}
bind 127.0.0.1
save ""
appendonly no
"#,
            self.port
        );
        
        let config_path = self.redis_path.parent().unwrap().join("redis.conf");
        std::fs::write(&config_path, config_content)?;

        // Start Redis process
        let child = Command::new(&self.redis_path)
            .arg(&config_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        info!("🔴 Redis server process started with PID: {:?}", child.id());
        
        self.process = Some(child);
        self.is_running.store(true, Ordering::Relaxed);
        
        // Give Redis a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        Ok(())
    }
    
    async fn start_system_redis(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("🔴 Attempting to start system Redis server");
        
        // Try to start system Redis (if available)
        match Command::new("redis-server")
            .arg("--port")
            .arg(self.port.to_string())
            .arg("--bind")
            .arg("127.0.0.1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                info!("🔴 System Redis server started with PID: {:?}", child.id());
                self.process = Some(child);
                self.is_running.store(true, Ordering::Relaxed);
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                Ok(())
            }
            Err(e) => {
                error!("🔴 Failed to start system Redis: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Wait for the server to be ready to accept connections
    pub async fn wait_for_ready(&self, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        
        while start.elapsed() < timeout {
            if self.is_running() {
                // Try to connect to verify it's actually ready
                match redis::Client::open(format!("redis://127.0.0.1:{}/", self.port)) {
                    Ok(client) => {
                        match client.get_connection() {
                            Ok(mut conn) => {
                                match redis::cmd("PING").query::<String>(&mut conn) {
                                    Ok(_) => {
                                        info!("🔴 Redis server is ready to accept connections");
                                        return true;
                                    }
                                    Err(_) => {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                        continue;
                                    }
                                }
                            }
                            Err(_) => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                continue;
                            }
                        }
                    }
                    Err(_) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        warn!("🔴 Redis server failed to become ready within {}ms", timeout_ms);
        false
    }

    pub fn stop(&mut self) {
        if let Some(mut process) = self.process.take() {
            info!("🔴 Stopping Redis server...");
            let _ = process.kill();
            self.is_running.store(false, Ordering::Relaxed);
        }
        
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Drop for EmbeddedRedisServer {
    fn drop(&mut self) {
        self.stop();
    }
}