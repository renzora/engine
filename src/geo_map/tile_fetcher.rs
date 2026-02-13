use std::path::PathBuf;
use std::sync::mpsc;

use super::tile::TileCoord;
use super::tile_cache::FetchedTile;

/// Fetch a single tile in a background thread.
/// Checks disk cache first, falls back to HTTP via ureq.
pub fn fetch_tile(
    coord: TileCoord,
    url: String,
    cache_dir: Option<PathBuf>,
    sender: mpsc::Sender<FetchedTile>,
) {
    std::thread::spawn(move || {
        // 1. Check disk cache
        if let Some(dir) = &cache_dir {
            let path = dir
                .join(coord.z.to_string())
                .join(coord.x.to_string())
                .join(format!("{}.png", coord.y));
            if path.exists() {
                if let Ok(bytes) = std::fs::read(&path) {
                    let _ = sender.send(FetchedTile { coord, bytes });
                    return;
                }
            }
        }

        // 2. HTTP fetch
        let result = ureq::get(&url)
            .set("User-Agent", "renzora-editor/1.0")
            .call();

        let response = match result {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Tile fetch failed for {}: {}", url, e);
                return;
            }
        };

        let mut bytes = Vec::new();
        if let Err(e) = std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes) {
            eprintln!("Tile read failed for {}: {}", url, e);
            return;
        }

        // 3. Write to disk cache
        if let Some(dir) = &cache_dir {
            let path = dir
                .join(coord.z.to_string())
                .join(coord.x.to_string());
            let _ = std::fs::create_dir_all(&path);
            let file_path = path.join(format!("{}.png", coord.y));
            let _ = std::fs::write(&file_path, &bytes);
        }

        // 4. Send to ECS
        let _ = sender.send(FetchedTile { coord, bytes });
    });
}
