use sqlx::{SqlitePool, Row, Column};
use serde::{Serialize, Deserialize};
use std::path::Path;
use log::{info, debug};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RenScript {
    pub id: String,
    pub name: String,
    pub path: String,
    pub directory: String,
    pub content: String,
    pub properties: Option<String>,
    pub compiled_js: Option<String>,
    pub last_modified: i64,
    pub compilation_status: String,
    pub compilation_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptSearchResult {
    pub name: String,
    pub path: String,
    pub directory: String,
    pub last_modified: i64,
}

pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub async fn new(database_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Create database directory if it doesn't exist
        if let Some(parent) = Path::new(database_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let database_url = format!("sqlite://{}", database_path);
        let pool = SqlitePool::connect(&database_url).await?;
        
        let manager = DatabaseManager { pool };
        manager.initialize_schema().await?;
        
        info!("📊 Database initialized at: {}", database_path);
        Ok(manager)
    }

    async fn initialize_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS renscripts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                directory TEXT NOT NULL,
                content TEXT NOT NULL,
                properties TEXT,
                compiled_js TEXT,
                last_modified INTEGER NOT NULL,
                compilation_status TEXT NOT NULL DEFAULT 'pending',
                compilation_error TEXT,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );
            "#
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_renscripts_name ON renscripts(name);
            CREATE INDEX IF NOT EXISTS idx_renscripts_directory ON renscripts(directory);
            CREATE INDEX IF NOT EXISTS idx_renscripts_last_modified ON renscripts(last_modified);
            CREATE INDEX IF NOT EXISTS idx_renscripts_compilation_status ON renscripts(compilation_status);
            "#
        )
        .execute(&self.pool)
        .await?;

        info!("📊 Database schema initialized");
        Ok(())
    }

    pub async fn search_scripts(&self, search_term: &str) -> Result<Vec<ScriptSearchResult>, sqlx::Error> {
        let search_pattern = format!("%{}%", search_term.to_lowercase());
        
        let rows = sqlx::query(
            r#"
            SELECT name, path, directory, last_modified 
            FROM renscripts 
            WHERE LOWER(name) LIKE ? OR LOWER(directory) LIKE ?
            ORDER BY name ASC
            "#
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<ScriptSearchResult> = rows.into_iter().map(|row| ScriptSearchResult {
            name: row.get("name"),
            path: row.get("path"),
            directory: row.get("directory"),
            last_modified: row.get("last_modified"),
        }).collect();

        debug!("🔍 Database search for '{}' returned {} results", search_term, results.len());
        Ok(results)
    }

    pub async fn get_all_scripts(&self) -> Result<Vec<ScriptSearchResult>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT name, path, directory, last_modified FROM renscripts ORDER BY name ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<ScriptSearchResult> = rows.into_iter().map(|row| ScriptSearchResult {
            name: row.get("name"),
            path: row.get("path"),
            directory: row.get("directory"),
            last_modified: row.get("last_modified"),
        }).collect();

        debug!("📊 Retrieved {} scripts from database", results.len());
        Ok(results)
    }

    pub async fn get_compilation_stats(&self) -> Result<serde_json::Value, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total,
                SUM(CASE WHEN compilation_status = 'success' THEN 1 ELSE 0 END) as successful,
                SUM(CASE WHEN compilation_status = 'error' THEN 1 ELSE 0 END) as errors,
                MAX(updated_at) as last_update
            FROM renscripts
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "total_scripts": row.get::<i64, _>("total"),
            "successful_compilations": row.get::<i64, _>("successful"),
            "compilation_errors": row.get::<i64, _>("errors"),
            "last_update": row.get::<i64, _>("last_update")
        }))
    }

    pub async fn execute_raw_query(&self, query: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        debug!("🔍 Executing raw SQL query: {}", query);
        
        // Validate query to prevent dangerous operations
        let query_lower = query.to_lowercase();
        let query_trimmed = query_lower.trim();
        if query_trimmed.starts_with("drop") || 
           query_trimmed.starts_with("delete") || 
           query_trimmed.starts_with("truncate") ||
           query_trimmed.starts_with("alter") ||
           query_trimmed.starts_with("create") ||
           query_trimmed.starts_with("insert") ||
           query_trimmed.starts_with("update") {
            return Err("Only SELECT queries are allowed for security".into());
        }

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let mut row_data = serde_json::Map::new();
            
            // Extract column names and values
            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();
                
                // Try to get the value as different types
                let value = if let Ok(val) = row.try_get::<String, _>(i) {
                    serde_json::Value::String(val)
                } else if let Ok(val) = row.try_get::<i64, _>(i) {
                    serde_json::Value::Number(serde_json::Number::from(val))
                } else if let Ok(val) = row.try_get::<f64, _>(i) {
                    serde_json::Value::Number(serde_json::Number::from_f64(val).unwrap_or(serde_json::Number::from(0)))
                } else if let Ok(val) = row.try_get::<bool, _>(i) {
                    serde_json::Value::Bool(val)
                } else {
                    serde_json::Value::Null
                };
                
                row_data.insert(column_name.to_string(), value);
            }
            
            results.push(serde_json::Value::Object(row_data));
        }

        Ok(serde_json::json!({
            "success": true,
            "rows": results,
            "count": results.len()
        }))
    }
}