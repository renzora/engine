use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

use super::tile::TileCoord;

const MAX_CACHED_TILES: usize = 256;

/// A fetched tile's raw image bytes + its coordinate
pub struct FetchedTile {
    pub coord: TileCoord,
    pub bytes: Vec<u8>,
}

/// In-memory tile cache with LRU eviction + disk cache support
#[derive(Resource)]
pub struct GeoTileCache {
    pub tiles: HashMap<TileCoord, Handle<Image>>,
    lru: VecDeque<TileCoord>,
    pub pending: HashSet<TileCoord>,
    pub sender: mpsc::Sender<FetchedTile>,
    receiver: Mutex<mpsc::Receiver<FetchedTile>>,
    pub cache_dir: Option<PathBuf>,
    pub disk_tile_count: usize,
}

impl GeoTileCache {
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            tiles: HashMap::new(),
            lru: VecDeque::new(),
            pending: HashSet::new(),
            sender,
            receiver: Mutex::new(receiver),
            cache_dir,
            disk_tile_count: 0,
        }
    }

    pub fn get(&mut self, coord: &TileCoord) -> Option<Handle<Image>> {
        if self.tiles.contains_key(coord) {
            self.lru.retain(|c| c != coord);
            self.lru.push_back(*coord);
            self.tiles.get(coord).cloned()
        } else {
            None
        }
    }

    pub fn insert(&mut self, coord: TileCoord, handle: Handle<Image>) {
        if self.tiles.len() >= MAX_CACHED_TILES {
            if let Some(evicted) = self.lru.pop_front() {
                self.tiles.remove(&evicted);
            }
        }
        self.lru.retain(|c| c != &coord);
        self.lru.push_back(coord);
        self.tiles.insert(coord, handle);
        self.pending.remove(&coord);
    }

    pub fn contains(&self, coord: &TileCoord) -> bool {
        self.tiles.contains_key(coord)
    }

    pub fn is_pending(&self, coord: &TileCoord) -> bool {
        self.pending.contains(coord)
    }

    pub fn mark_pending(&mut self, coord: TileCoord) {
        self.pending.insert(coord);
    }

    /// Try to receive a fetched tile from the background thread channel
    pub fn try_recv(&self) -> Option<FetchedTile> {
        let Ok(rx) = self.receiver.lock() else { return None };
        rx.try_recv().ok()
    }

    pub fn clear_memory(&mut self) {
        self.tiles.clear();
        self.lru.clear();
    }

    pub fn disk_path(&self, coord: &TileCoord) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|dir| {
            dir.join(coord.z.to_string())
                .join(coord.x.to_string())
                .join(format!("{}.png", coord.y))
        })
    }

    pub fn clear_disk_cache(&mut self) {
        if let Some(dir) = &self.cache_dir {
            let _ = std::fs::remove_dir_all(dir);
            let _ = std::fs::create_dir_all(dir);
            self.disk_tile_count = 0;
        }
    }

    pub fn count_disk_tiles(&mut self) {
        let Some(dir) = &self.cache_dir else { return };
        let mut count = 0usize;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for z_entry in entries.flatten() {
                if let Ok(x_entries) = std::fs::read_dir(z_entry.path()) {
                    for x_entry in x_entries.flatten() {
                        if let Ok(y_entries) = std::fs::read_dir(x_entry.path()) {
                            count += y_entries.count();
                        }
                    }
                }
            }
        }
        self.disk_tile_count = count;
    }

    pub fn estimate_disk_usage(&self) -> u64 {
        self.disk_tile_count as u64 * 40_000
    }
}
