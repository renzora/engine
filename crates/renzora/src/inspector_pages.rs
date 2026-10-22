//! The Inspector's pages: the one list of component categories every editor
//! surface groups by.
//!
//! The Inspector shows one page of component cards at a time, picked from a
//! rail down its left edge, and Add Component groups what it offers the same
//! way. Both read this table, so a component lands on the same page in both
//! places and a page is the same colour and icon everywhere.
//!
//! It is here, outside the `editor` feature, because ember (which builds Add
//! Component's picker) links the contract crate without that feature, and the
//! picker has to agree with the Inspector about what the pages are.
//!
//! Registrations name their page through `InspectorEntry::category`, a plain
//! string, so a plugin built before this table existed still lands somewhere
//! sensible: [`InspectorPage::for_entry`] maps the older category names
//! (`"general"`, `"component"`, `"effects"`, `"rendering"` for sky and fog,
//! ...) onto the pages, and anything it does not recognise goes to
//! [`InspectorPage::Object`].

/// One page of the Inspector, in rail order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InspectorPage {
    /// Transform, and anything that describes the entity itself.
    Object,
    /// Meshes, materials, decals, LODs, text, splats, sprites.
    Rendering,
    /// Lights, light probes, ray-traced GI.
    Lighting,
    /// The camera and the image effects attached to it: bloom, tonemapping,
    /// anti-aliasing, depth of field, motion blur.
    Camera,
    /// The world around the scene: sky, atmosphere, clouds, fog, water, wind.
    Environment,
    /// Screen-space and shader post effects: SSAO, SSR, post-effect stacks.
    PostProcess,
    Physics,
    Animation,
    /// Scripts and blueprints.
    Scripting,
    Audio,
    Navigation,
    Networking,
    Ui,
    Particles,
}

impl InspectorPage {
    /// Every page, in the order the rail shows them.
    pub const ALL: [InspectorPage; 14] = [
        InspectorPage::Object,
        InspectorPage::Rendering,
        InspectorPage::Lighting,
        InspectorPage::Camera,
        InspectorPage::Environment,
        InspectorPage::PostProcess,
        InspectorPage::Physics,
        InspectorPage::Animation,
        InspectorPage::Scripting,
        InspectorPage::Audio,
        InspectorPage::Navigation,
        InspectorPage::Networking,
        InspectorPage::Ui,
        InspectorPage::Particles,
    ];

    /// The category string a registration uses to put itself on this page.
    pub fn id(self) -> &'static str {
        match self {
            InspectorPage::Object => "object",
            InspectorPage::Rendering => "rendering",
            InspectorPage::Lighting => "lighting",
            InspectorPage::Camera => "camera",
            InspectorPage::Environment => "environment",
            InspectorPage::PostProcess => "post_process",
            InspectorPage::Physics => "physics",
            InspectorPage::Animation => "animation",
            InspectorPage::Scripting => "scripting",
            InspectorPage::Audio => "audio",
            InspectorPage::Navigation => "navigation",
            InspectorPage::Networking => "networking",
            InspectorPage::Ui => "ui",
            InspectorPage::Particles => "particles",
        }
    }

    /// The English name. Translated through `inspector.page.<id>`.
    pub fn label(self) -> &'static str {
        match self {
            InspectorPage::Object => "Object",
            InspectorPage::Rendering => "Rendering",
            InspectorPage::Lighting => "Lighting",
            InspectorPage::Camera => "Camera",
            InspectorPage::Environment => "Environment",
            InspectorPage::PostProcess => "Post Process",
            InspectorPage::Physics => "Physics",
            InspectorPage::Animation => "Animation",
            InspectorPage::Scripting => "Scripting",
            InspectorPage::Audio => "Audio",
            InspectorPage::Navigation => "Navigation",
            InspectorPage::Networking => "Networking",
            InspectorPage::Ui => "UI",
            InspectorPage::Particles => "Particles",
        }
    }

    /// The localized name.
    pub fn display_name(self) -> String {
        crate::lang::t_or(&format!("inspector.page.{}", self.id()), self.label())
    }

    /// The Phosphor icon on its rail button.
    pub fn icon(self) -> &'static str {
        match self {
            InspectorPage::Object => "arrows-out-cardinal",
            InspectorPage::Rendering => "paint-brush",
            InspectorPage::Lighting => "sun",
            InspectorPage::Camera => "video-camera",
            InspectorPage::Environment => "mountains",
            InspectorPage::PostProcess => "magic-wand",
            InspectorPage::Physics => "atom",
            InspectorPage::Animation => "film-strip",
            InspectorPage::Scripting => "code",
            InspectorPage::Audio => "speaker-high",
            InspectorPage::Navigation => "compass",
            InspectorPage::Networking => "globe",
            InspectorPage::Ui => "layout",
            InspectorPage::Particles => "sparkle",
        }
    }

    /// The page's colour, for its rail icon and Add Component's category dot.
    ///
    /// Fourteen hues cannot all be far apart, so neighbours that share a
    /// family (Object and Camera are both blue) also differ in lightness, and
    /// every button carries an icon of its own as well.
    pub fn hue(self) -> (u8, u8, u8) {
        match self {
            InspectorPage::Object => (150, 200, 255),
            InspectorPage::Rendering => (180, 156, 255),
            InspectorPage::Lighting => (255, 210, 120),
            InspectorPage::Camera => (103, 222, 245),
            InspectorPage::Environment => (150, 215, 120),
            InspectorPage::PostProcess => (232, 140, 240),
            InspectorPage::Physics => (255, 165, 110),
            InspectorPage::Animation => (255, 128, 160),
            InspectorPage::Scripting => (90, 220, 190),
            InspectorPage::Audio => (200, 140, 230),
            InspectorPage::Navigation => (200, 225, 110),
            InspectorPage::Networking => (165, 175, 205),
            InspectorPage::Ui => (255, 185, 205),
            InspectorPage::Particles => (245, 120, 95),
        }
    }

    /// The page whose [`id`](Self::id) is `id`.
    pub fn from_id(id: &str) -> Option<InspectorPage> {
        InspectorPage::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The page a registration belongs on, from its `category` and `type_id`.
    ///
    /// The category decides, through its current name or an older one. The
    /// `type_id` only matters for a category that used to hold things that
    /// now live elsewhere: sky, fog and the screen-space passes were all once
    /// filed under `"rendering"`, and a marketplace plugin built then still
    /// says so.
    pub fn for_entry(category: &str, type_id: &str) -> InspectorPage {
        let cat = category.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        if let Some(page) = InspectorPage::from_id(&cat) {
            if page == InspectorPage::Rendering {
                if let Some(moved) = moved_out_of_rendering(type_id) {
                    return moved;
                }
            }
            return page;
        }
        match cat.as_str() {
            "light" => InspectorPage::Lighting,
            "postprocess" | "post_processing" => InspectorPage::PostProcess,
            "script" | "gameplay" => InspectorPage::Scripting,
            "network" | "multiplayer" => InspectorPage::Networking,
            "effects" | "particle" | "vfx" => InspectorPage::Particles,
            "sound" => InspectorPage::Audio,
            "mesh" | "material" | "nodes2d" | "nodes_2d" | "2d" => InspectorPage::Rendering,
            // "transform", "general", "basic", "component", "" and anything
            // unknown: the entity's own page.
            _ => InspectorPage::Object,
        }
    }
}

/// Where a component once filed under `"rendering"` belongs now, judged from
/// its `type_id`. `None` keeps it on Rendering.
fn moved_out_of_rendering(type_id: &str) -> Option<InspectorPage> {
    let t = type_id.to_ascii_lowercase();
    const ENVIRONMENT: [&str; 9] = [
        "sky", "atmosphere", "cloud", "fog", "water", "ocean", "wind", "star", "weather",
    ];
    if t == "environment_map" || ENVIRONMENT.iter().any(|k| t.contains(k)) {
        return Some(InspectorPage::Environment);
    }
    if t.contains("ssao") || t.contains("ssr") || t.contains("post_effect") {
        return Some(InspectorPage::PostProcess);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::InspectorPage as P;

    #[test]
    fn current_ids_round_trip() {
        for p in P::ALL {
            assert_eq!(P::for_entry(p.id(), "anything"), p);
        }
    }

    #[test]
    fn older_categories_land_on_their_new_page() {
        assert_eq!(P::for_entry("transform", "transform"), P::Object);
        assert_eq!(P::for_entry("component", "terrain_data"), P::Object);
        assert_eq!(P::for_entry("effects", "particle_system"), P::Particles);
        assert_eq!(P::for_entry("Audio", "audio_player"), P::Audio);
        assert_eq!(P::for_entry("rendering", "clouds"), P::Environment);
        assert_eq!(P::for_entry("rendering", "world_env_ssao"), P::PostProcess);
        assert_eq!(P::for_entry("rendering", "mesh_lod"), P::Rendering);
    }

    #[test]
    fn an_environment_map_light_stays_with_the_lights() {
        // Bevy's image-based light probe is a light, not the sky behind it.
        assert_eq!(P::for_entry("lighting", "environment_map_light"), P::Lighting);
        assert_eq!(P::for_entry("rendering", "environment_map"), P::Environment);
    }
}
