//! 3D text as a standalone C-ABI plugin.
//!
//! A [`Text3d`] entity renders a string as real triangulated glyph geometry —
//! the font's actual outlines, triangulated with holes and optionally extruded
//! into a solid — so it sits in the scene like any other object: lit, movable,
//! and correct from every angle.
//!
//! ## What this is a port of, and what it left behind
//!
//! `crates/renzora_text3d` had two modes. **Mesh mode** is here in full: it is
//! pure geometry from `ttf-parser` + `lyon`, neither of which has ever heard of
//! Bevy, so it crossed the boundary unchanged.
//!
//! **Flat mode** did not. It rasterizes glyphs into an SDF atlas and draws
//! textured quads — cheap, and the better choice for a lot of UI — but it needs
//! `Assets<Image>` to build that atlas, and the ABI has no way to create or
//! write a texture. That is the single capability standing between this and a
//! complete port, and it is the same one `pool_water` is waiting on.
//!
//! ## Why the component can exist at all
//!
//! It holds two strings, which plugin components could not do until
//! [`Str256`] — 252 bytes of inline UTF-8 plus a length. A `String` is
//! impossible here: component storage is allocated by the host from a layout the
//! plugin declares, and anything with a destructor is refused outright.

mod outline;

use renzora_plugin::prelude::*;
use std::collections::HashMap;

/// Font used when [`Text3d::font`] is empty.
///
/// Embedded rather than loaded, because mesh mode needs real outline bytes and
/// there is no asset server on this side of the boundary — a plugin reads files
/// itself or ships them.
const DEFAULT_FONT: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// How many `Text3d` entities can exist at once.
///
/// `add_mesh_data` is init-only, so every mesh the plugin will ever write has to
/// be created during `build`. Each unused slot is an empty mesh.
const TEXT_POOL: usize = 64;

/// A string rendered as 3D geometry at this entity's transform.
#[derive(Component)]
#[component(name = "Text 3D")]
#[repr(C)]
pub struct Text3d {
    /// The text to display. Capped at 252 bytes — see [`Str256`].
    pub text: Str256,
    /// Path to a `.ttf`/`.otf`, or empty for the built-in font.
    ///
    /// Resolved by the plugin with `std::fs`, relative to the editor's working
    /// directory. There is no asset server across the boundary, so this cannot
    /// take an asset-relative path the way the engine version did.
    pub font: Str256,
    /// Em size in pixels; scaled to world units on build.
    #[field(min = 1.0, max = 512.0, speed = 1.0)]
    pub size: f32,
    /// Extrusion depth in world units. 0 gives a flat filled outline.
    #[field(min = 0.0, max = 2.0, speed = 0.005)]
    pub depth: f32,
    /// Linear RGB.
    #[field(speed = 0.005)]
    pub color: Vec3,

    /// The mesh slot this text was last built into, or -1 for none.
    ///
    /// Kept on the component rather than in plugin memory so it survives a hot
    /// reload — the same reasoning as `plugins/hair`, which stranded its render
    /// entities until it did this. Here it also means a reload reuses the slot
    /// instead of leaking one per build.
    #[field(skip)]
    pub slot: i32,
}

impl Default for Text3d {
    fn default() -> Self {
        Self {
            text: Str256::new("Text").unwrap_or(Str256::EMPTY),
            font: Str256::EMPTY,
            size: 64.0,
            depth: 0.05,
            color: Vec3 { x: 0.9, y: 0.9, z: 0.9 },
            slot: -1,
        }
    }
}

impl Text3d {
    /// Hash of everything the geometry depends on. Colour is excluded — it is a
    /// vertex attribute, rewritten every rebuild but never a reason for one.
    fn signature(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in self.text.as_bytes().iter().chain(self.font.as_bytes()) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for v in [self.size, self.depth] {
            h ^= v.to_bits() as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// Plugin-side state. Not an ECS resource — see `plugins/hair` for why a `Vec`
/// cannot live in one.
struct State {
    /// Mesh slots not in use.
    free: Vec<renzora_plugin::sys::AssetHandle>,
    /// Every slot by index, so a `Text3d` that already owns one can look it up
    /// after a reload wiped everything else.
    all: Vec<renzora_plugin::sys::AssetHandle>,
    material: renzora_plugin::sys::AssetHandle,
    /// Last-built signature per entity, so an unchanged string is not
    /// re-triangulated every frame.
    built: HashMap<u64, u64>,
    /// Font files read from disk, keyed by path. Parsing is cheap but reading is
    /// not, and a rebuild happens on every keystroke while editing the string.
    fonts: HashMap<String, Vec<u8>>,
}

static STATE: std::sync::Mutex<Option<State>> = std::sync::Mutex::new(None);

fn update_text(mut q: Query<(Entity, &mut Text3d, &Transform)>, meshes: Meshes, mut cmds: Commands) {
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };

    let mut live: Vec<u64> = Vec::new();

    for (entity, text, transform) in &mut q {
        live.push(entity.0);
        let signature = text.signature();
        if state.built.get(&entity.0) == Some(&signature) {
            continue;
        }

        // Claim a slot, reusing whatever the component already owns. After a
        // reload `built` is empty but `slot` is not, which is what keeps a
        // reload from leaking a slot per entity.
        let handle = if text.slot >= 0 {
            match state.all.get(text.slot as usize).copied() {
                Some(h) => h,
                None => continue,
            }
        } else {
            let Some(h) = state.free.pop() else {
                error("text3d: no free mesh slots left");
                continue;
            };
            text.slot = h.0 as i32;
            h
        };

        let bytes = load_font(state, text.font.as_str());
        let Some(glyphs) =
            outline::build_outline_mesh(bytes, text.text.as_str(), text.size, text.depth)
        else {
            // Nothing outlined — an empty string, or all-unmapped characters.
            // Write an empty mesh so the previous text disappears rather than
            // lingering, and record the signature so this is not retried every
            // frame.
            meshes.write(handle, &[], None, None, None, None);
            state.built.insert(entity.0, signature);
            continue;
        };

        let colors = vec![[text.color.x, text.color.y, text.color.z, 1.0]; glyphs.positions.len()];
        meshes.write(
            handle,
            &glyphs.positions,
            Some(&glyphs.normals),
            Some(&glyphs.uvs),
            Some(&glyphs.indices),
            Some(&colors),
        );

        // The mesh goes on the entity itself rather than a child: the geometry
        // is in local space around the origin and the boundary exposes no
        // parenting, so a separate entity could not follow this one.
        //
        // The transform passed back is the one just read, so re-issuing this on
        // every rebuild leaves the user's placement untouched.
        cmds.entity(entity)
            .make_renderable(handle, state.material, *transform);

        state.built.insert(entity.0, signature);
    }

    retire(state, &live);
}

/// Forget entities that no longer have `Text3d`, returning their slots.
///
/// No `RemovedComponents` across the boundary, so absence is the signal. The
/// mesh is left as it is: the entity carrying it is gone, and the slot's next
/// owner overwrites it.
fn retire(state: &mut State, live: &[u64]) {
    if state.built.len() == live.len() {
        return;
    }
    let dead: Vec<u64> = state
        .built
        .keys()
        .copied()
        .filter(|k| !live.contains(k))
        .collect();
    for key in dead {
        state.built.remove(&key);
    }
}

/// Font bytes for `path`, or the built-in font.
///
/// Cached because a rebuild fires on every keystroke while the string is being
/// edited, and re-reading a megabyte of TTF per frame would be felt.
fn load_font<'a>(state: &'a mut State, path: &str) -> &'a [u8] {
    if path.is_empty() {
        return DEFAULT_FONT;
    }
    if !state.fonts.contains_key(path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                state.fonts.insert(path.to_string(), bytes);
            }
            Err(e) => {
                error(&format!("text3d: cannot read font `{path}`: {e}"));
                // Cache the failure as empty so the error is logged once rather
                // than every frame; `build_outline_mesh` rejects it and the
                // entity falls back to showing nothing.
                state.fonts.insert(path.to_string(), Vec::new());
            }
        }
    }
    match state.fonts.get(path) {
        Some(b) if !b.is_empty() => b,
        _ => DEFAULT_FONT,
    }
}

pub struct Text3dPlugin;

impl Plugin for Text3dPlugin {
    fn build(&self, app: &mut App) {
        // Every mesh has to exist now — `add_mesh_data` needs the init-time host
        // handle. Each starts as a degenerate triangle because a mesh with no
        // positions is refused, and is overwritten the first time its slot is
        // claimed.
        let seed = [Vec3 { x: 0.0, y: 0.0, z: 0.0 }; 3];
        let all: Vec<_> = (0..TEXT_POOL)
            .map(|_| app.add_mesh_data(&seed, None, None, Some(&[0, 1, 2])))
            .collect();
        // White, so the per-vertex colours carry the text colour unmodified.
        let material = app.add_material([1.0, 1.0, 1.0, 1.0]);

        app.register_component::<Text3d>()
            .add_systems(Update, update_text);

        if let Ok(mut s) = STATE.lock() {
            *s = Some(State {
                free: all.clone(),
                all,
                material,
                built: HashMap::new(),
                fonts: HashMap::new(),
            });
        }
    }
}

renzora_plugin::add!(Text3dPlugin);
