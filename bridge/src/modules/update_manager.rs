use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Dev,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Stable => write!(f, "stable"),
            Channel::Dev => write!(f, "dev"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: Option<u32>,
    pub channel: Channel,
}

impl Version {
    pub fn new_stable(major: u32) -> Self {
        Version {
            major,
            minor: None,
            channel: Channel::Stable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub github_owner: String,
    pub github_repo: String,
    pub current_version: Version,
    pub update_channel: Channel,
    pub auto_update: bool,
    pub check_interval_hours: u32,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig {
            github_owner: "renzora".to_string(),
            github_repo: "engine".to_string(),
            current_version: Version::new_stable(1),
            update_channel: Channel::Stable,
            auto_update: false,
            check_interval_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub published_at: String,
    pub body: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub commit: CommitInfo,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub message: String,
    pub author: CommitAuthor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_stable_version: Option<String>,
    pub latest_dev_version: Option<String>,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub channel: Channel,
}

lazy_static::lazy_static! {
    static ref UPDATE_CONFIG: RwLock<UpdateConfig> = RwLock::new(UpdateConfig::default());
    static ref LAST_UPDATE_CHECK: RwLock<Option<UpdateCheckResult>> = RwLock::new(None);
}

pub fn get_current_config() -> UpdateConfig {
    UPDATE_CONFIG.read().unwrap().clone()
}

pub fn set_update_channel(channel: Channel) -> Result<(), String> {
    let mut config = UPDATE_CONFIG.write().map_err(|e| format!("Failed to lock config: {}", e))?;
    config.update_channel = channel;
    Ok(())
}

pub fn get_last_update_check() -> Option<UpdateCheckResult> {
    LAST_UPDATE_CHECK.read().unwrap().clone()
}

pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let config = get_current_config();
    
    let client = reqwest::Client::new();
    let mut result = UpdateCheckResult {
        current_version: format!("r{}", config.current_version.major),
        latest_stable_version: None,
        latest_dev_version: None,
        update_available: false,
        download_url: None,
        release_notes: None,
        channel: config.update_channel.clone(),
    };
    
    match config.update_channel {
        Channel::Stable => {
            // Check GitHub releases for stable channel
            let url = format!(
                "https://api.github.com/repos/{}/{}/releases",
                config.github_owner, config.github_repo
            );
            
            let response = client
                .get(&url)
                .header("User-Agent", "RenzoraEngine/1.0")
                .send()
                .await
                .map_err(|e| format!("Failed to fetch releases: {}", e))?;
            
            if response.status().is_success() {
                let releases: Vec<GitHubRelease> = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse releases: {}", e))?;
                
                // Find latest stable release (non-prerelease)
                if let Some(latest) = releases.iter().find(|r| !r.prerelease) {
                    result.latest_stable_version = Some(latest.tag_name.clone());
                    result.download_url = Some(latest.html_url.clone());
                    result.release_notes = latest.body.clone();
                    
                    // Check if update is available
                    if let Some(version_num) = parse_version_tag(&latest.tag_name) {
                        result.update_available = version_num > config.current_version.major;
                    }
                }
            }
        }
        Channel::Dev => {
            // Check GitHub commits for dev channel
            let url = format!(
                "https://api.github.com/repos/{}/{}/commits",
                config.github_owner, config.github_repo
            );
            
            let response = client
                .get(&url)
                .header("User-Agent", "RenzoraEngine/1.0")
                .send()
                .await
                .map_err(|e| format!("Failed to fetch commits: {}", e))?;
            
            if response.status().is_success() {
                let commits: Vec<GitHubCommit> = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse commits: {}", e))?;
                
                // Use the latest commit as the dev version
                if let Some(latest_commit) = commits.first() {
                    let short_sha = &latest_commit.sha[..7];
                    result.latest_dev_version = Some(format!("dev-{}", short_sha));
                    result.download_url = Some(latest_commit.html_url.clone());
                    result.release_notes = Some(format!(
                        "Latest commit: {}\nAuthor: {}\nDate: {}",
                        latest_commit.commit.message,
                        latest_commit.commit.author.name,
                        latest_commit.commit.author.date
                    ));
                    result.update_available = true; // Always show updates available for dev
                }
            }
        }
    }
    
    // Cache the result
    if let Ok(mut last_check) = LAST_UPDATE_CHECK.write() {
        *last_check = Some(result.clone());
    }
    
    Ok(result)
}

fn parse_version_tag(tag: &str) -> Option<u32> {
    // Parse tags like "r1", "v1", "1.0", etc.
    let tag = tag.trim_start_matches('v').trim_start_matches('r');
    tag.split('.').next()?.parse().ok()
}


