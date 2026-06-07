//! Inspector entry for the Material component.
//!
//! Registered automatically by `MaterialEditorPlugin`.

use bevy::prelude::*;
use renzora_editor_framework::InspectorEntry;
use renzora_shader::material::material_ref::MaterialRef;

/// Image extensions accepted for auto-material creation.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp",
];

/// How deep we recurse when scanning the project for `.material` files. Six
/// levels covers `models/<asset>/materials/` plus a couple of hand-organized
/// subfolders on top of `assets/materials/`. Models with deeper nesting are
/// rare and the user can drop directly to bind those.
const MATERIAL_SCAN_MAX_DEPTH: usize = 6;

pub fn material_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "material_ref",
        display_name: "Material",
        icon: "paint-brush",
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<MaterialRef>(entity).is_some()
                || world
                    .get::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>(entity)
                    .is_some()
                || world.get::<Mesh3d>(entity).is_some()
        },
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<MaterialRef>();
            world
                .entity_mut(entity)
                .remove::<renzora_shader::material::resolver::MaterialResolved>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        // Native (bevy_ui) drawer registered via `register_native_inspector_ui`
        // (see `native_material_ref`). No egui custom UI.
    }
}

/// Convert a [`PinValue`] (master default) into a [`ParamValue`]
/// (override-storage) when the kinds happen to align. Returns `None`
/// for `PinValue` variants that don't appear in `ParamValue` (string,
/// texture path, none) — those are filtered upstream because their
/// `ParamKind` isn't a known kind.
pub(crate) fn pin_to_param(
    pin: &renzora_shader::material::graph::PinValue,
) -> Option<renzora_shader::material::material_ref::ParamValue> {
    use renzora_shader::material::graph::PinValue;
    use renzora_shader::material::material_ref::ParamValue;
    Some(match pin {
        PinValue::Float(f) => ParamValue::Float(*f),
        PinValue::Vec2(v) => ParamValue::Vec2(*v),
        PinValue::Vec3(v) => ParamValue::Vec3(*v),
        PinValue::Vec4(v) => ParamValue::Vec4(*v),
        PinValue::Color(c) => ParamValue::Color(*c),
        PinValue::Int(i) => ParamValue::Int(*i),
        PinValue::Bool(b) => ParamValue::Bool(*b),
        PinValue::TexturePath(_) | PinValue::String(_) | PinValue::None => return None,
    })
}

/// Fallback param value used when the master's authored default isn't
/// representable as a [`ParamValue`] (string, texture path, etc.) —
/// exotic cases that show up only when a graph is mid-edit. The
/// inspector renders zero-equivalents so the widget has something to
/// show; the override map stays untouched until the user actually
/// edits the value.
pub(crate) fn default_param_value(
    kind: renzora_shader::material::codegen::ParamKind,
) -> renzora_shader::material::material_ref::ParamValue {
    use renzora_shader::material::codegen::ParamKind;
    use renzora_shader::material::material_ref::ParamValue;
    match kind {
        ParamKind::Float => ParamValue::Float(0.0),
        ParamKind::Color => ParamValue::Color([1.0, 1.0, 1.0, 1.0]),
        ParamKind::Vec2 => ParamValue::Vec2([0.0, 0.0]),
        ParamKind::Vec3 => ParamValue::Vec3([0.0, 0.0, 0.0]),
        ParamKind::Vec4 => ParamValue::Vec4([0.0, 0.0, 0.0, 0.0]),
        ParamKind::Bool => ParamValue::Bool(false),
    }
}

/// Walk the project root for `.material` files and return their
/// `(asset_relative_path, absolute_path)` pairs sorted alphabetically.
///
/// Bounded to [`MATERIAL_SCAN_MAX_DEPTH`] levels to keep the scan cheap. The
/// browse popup rebuilds this list every frame it's open — for a project
/// with hundreds of materials this is well under a millisecond on a SSD,
/// and avoiding a cache means added/renamed files show up immediately.
pub(crate) fn find_material_files(project_root: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(project_root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip dotfiles / hidden dirs (.git, .vscode) and target dirs —
            // they're noisy and never contain user-authored materials.
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" {
                    continue;
                }
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if depth + 1 < MATERIAL_SCAN_MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if ft.is_file()
                && matches!(path.extension().and_then(|e| e.to_str()), Some("material"))
            {
                if let Ok(rel) = path.strip_prefix(project_root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    out.push((rel_str, path.to_string_lossy().to_string()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
