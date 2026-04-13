//! Texture path resolution and embedded image handling.


/// Resolve a texture file path relative to the USD file's directory.
pub fn resolve_texture_path(usd_dir: &str, tex_path: &str) -> String {
    if tex_path.starts_with('/') || tex_path.contains(':') {
        // Absolute path — use as-is
        return tex_path.to_string();
    }

    // Strip any leading ./ from the texture path
    let cleaned = tex_path.strip_prefix("./").unwrap_or(tex_path);

    if usd_dir.is_empty() {
        cleaned.to_string()
    } else {
        format!("{}/{}", usd_dir.trim_end_matches('/'), cleaned)
    }
}
