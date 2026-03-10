//! Texture thumbnail management for material graph nodes.
//!
//! Watches texture nodes in the current MaterialGraph, loads referenced images,
//! and registers them with egui so they can be displayed as node thumbnails.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_material::graph::PinValue;

use crate::MaterialEditorState;

/// Resource mapping texture asset paths → egui TextureIds for node thumbnails.
#[derive(Resource, Default)]
pub struct NodeThumbnails {
    /// Path → (bevy image handle, egui texture id).
    pub entries: HashMap<String, ThumbnailEntry>,
}

pub struct ThumbnailEntry {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<egui::TextureId>,
}

impl NodeThumbnails {
    /// Build a TextureThumbnailMap for the graph editor sync.
    pub fn to_map(&self) -> HashMap<String, egui::TextureId> {
        self.entries
            .iter()
            .filter_map(|(path, entry)| {
                entry.texture_id.map(|id| (path.clone(), id))
            })
            .collect()
    }
}

/// System that scans material graph nodes for texture paths,
/// loads them if needed, and registers with egui.
fn update_node_thumbnails(
    editor_state: Res<MaterialEditorState>,
    asset_server: Res<AssetServer>,
    mut thumbnails: ResMut<NodeThumbnails>,
    mut user_textures: ResMut<EguiUserTextures>,
    images: Res<Assets<Image>>,
) {
    // Collect all texture paths from the current graph
    let mut needed_paths: Vec<String> = Vec::new();
    for node in &editor_state.graph.nodes {
        if let Some(PinValue::TexturePath(ref path)) = node.input_values.get("texture") {
            if !path.is_empty() {
                needed_paths.push(path.clone());
            }
        }
    }

    // Remove entries no longer needed
    thumbnails.entries.retain(|path, _| needed_paths.contains(path));

    // Load new textures and register with egui
    for path in &needed_paths {
        if thumbnails.entries.contains_key(path) {
            // Already tracked — check if we need to register the texture id
            let entry = thumbnails.entries.get_mut(path).unwrap();
            if entry.texture_id.is_none() {
                // Check if the image is loaded
                if images.contains(&entry.image_handle) {
                    user_textures.add_image(EguiTextureHandle::Strong(entry.image_handle.clone()));
                    entry.texture_id = user_textures.image_id(entry.image_handle.id());
                }
            }
            continue;
        }

        // New texture — start loading
        let owned_path: String = path.clone();
        let image_handle: Handle<Image> = asset_server.load(owned_path);
        let texture_id = if images.contains(&image_handle) {
            user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
            user_textures.image_id(image_handle.id())
        } else {
            None
        };

        thumbnails.entries.insert(
            path.clone(),
            ThumbnailEntry {
                image_handle,
                texture_id,
            },
        );
    }
}

pub struct NodeThumbnailPlugin;

impl Plugin for NodeThumbnailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NodeThumbnails>()
            .add_systems(Update, update_node_thumbnails);
    }
}
