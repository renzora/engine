//! Feature-gated `deny_component` chains for the scene-save builders.
//!
//! Four save chains need the same denies, and `#[cfg]` cannot sit on a link in
//! a method chain — so each gate lives here as a trait method instead of being
//! duplicated as a chain-splitting `let` at every call site. When a capability
//! is stripped by the lean exporter the component types do not exist at all, so
//! the stripped body is a genuine no-op rather than a behaviour change.

// Only the 3D and terrain denies name prelude types; the rest use full paths.
#[cfg(any(feature = "render_3d", feature = "terrain"))]
use bevy::prelude::*;
use renzora_bsn::DynamicSceneBuilder;

/// Chainable stand-in for the two `bevy_ui` camera-target denies.
pub(crate) trait DenyUiCameraTargets {
    fn deny_ui_camera_targets(self) -> Self;
}

impl DenyUiCameraTargets for DynamicSceneBuilder<'_> {
    /// `UiTargetCamera` holds an `Entity` reference that doesn't remap across
    /// loads (e.g. an editor-only play-mode camera), and `ComputedUiTargetCamera`
    /// is a runtime-derived mirror — persisting either makes UI render to a dead
    /// entity in the runtime and silently disappear.
    ///
    /// `IsDefaultUiCamera` goes too. A scene loaded into the editor shares its
    /// world with the editor's own interface, and a camera carrying it there
    /// takes every untargeted UI root with it, the editor's included: the
    /// editor's chrome vanished the moment a game script's UI camera was
    /// mirrored in during Play. A game that wants one marks it from a script.
    ///
    /// And everything bevy_ui and bevy_text derive from the authored node and
    /// font every frame: the computed layout (`ComputedNode`,
    /// `UiGlobalTransform`, `ComputedStackIndex`, `ComputedUiRenderTargetInfo`),
    /// text measurement (`ContentSize`) and the resolved font size (`EmSize`).
    /// Saved, they are only stale numbers; mirrored by the live link they were
    /// worse: the editor lays its copy of the game's UI out at another size,
    /// so its computed values differ, and sending them back made the game's
    /// UI jump between the two layouts every few frames.
    #[cfg(feature = "ui")]
    fn deny_ui_camera_targets(self) -> Self {
        self.deny_component::<bevy::ui::UiTargetCamera>()
            .deny_component::<bevy::ui::ComputedUiTargetCamera>()
            .deny_component::<bevy::ui::IsDefaultUiCamera>()
            .deny_component::<bevy::ui::ComputedNode>()
            .deny_component::<bevy::ui::UiGlobalTransform>()
            .deny_component::<bevy::ui::ComputedStackIndex>()
            .deny_component::<bevy::ui::ComputedUiRenderTargetInfo>()
            .deny_component::<bevy::ui::ContentSize>()
            .deny_component::<bevy::text::EmSize>()
    }

    #[cfg(not(feature = "ui"))]
    fn deny_ui_camera_targets(self) -> Self {
        self
    }
}

/// Conditionally-compiled `deny_component` calls for the optional `animation`
/// and `terrain` subsystems. When the lean exporter strips those crates, their
/// component types don't exist, so the deny is a no-op — but the call sites in
/// the three save chains stay identical, keeping them readable instead of
/// breaking each chain apart with an inline `#[cfg]`.
pub(crate) trait DenyOptionalSubsystems: Sized {
    fn deny_animation_state(self) -> Self;
    fn deny_terrain_material(self) -> Self;
    fn deny_render_3d_materials(self) -> Self;
    fn deny_network_components(self) -> Self;
}

impl DenyOptionalSubsystems for DynamicSceneBuilder<'_> {
    // The id the server hands out for one session. Everything else networked
    // (`Networked`, `NetworkOwner`, `NetworkTransform`, the player spawner) is
    // authored and must save: `Networked` used to be denied here too, which
    // made it impossible to mark anything in a scene for replication.
    #[cfg(feature = "networking")]
    fn deny_network_components(self) -> Self {
        self.deny_component::<renzora_network::NetworkId>()
    }
    #[cfg(not(feature = "networking"))]
    fn deny_network_components(self) -> Self {
        self
    }

    // Bevy's own playback components as well as ours, because the two strip
    // together: the `animation` capability drops `renzora_animation` AND bevy's
    // `bevy_animation` feature, so with it off neither set of types exists.
    // These three used to sit unguarded at all three call sites, which is what
    // kept `bevy_animation` — and `blake3` behind it — pinned into every export,
    // capability or no capability.
    #[cfg(feature = "animation")]
    fn deny_animation_state(self) -> Self {
        self
            // Ephemeral playback state; must rebuild on load.
            .deny_component::<bevy::animation::AnimationPlayer>()
            .deny_component::<bevy::animation::transition::AnimationTransitions>()
            // `AnimatedBy` stores an Entity reference that doesn't remap across
            // scene loads — reconstructed by the animator rehydration.
            .deny_component::<bevy::animation::AnimatedBy>()
            // AnimatorReadState is a runtime mirror — rebuilt each frame.
            .deny_component::<renzora_animation::AnimatorReadState>()
    }
    #[cfg(not(feature = "animation"))]
    fn deny_animation_state(self) -> Self {
        self
    }

    #[cfg(feature = "terrain")]
    fn deny_terrain_material(self) -> Self {
        self.deny_component::<MeshMaterial3d<renzora_terrain::material::TerrainCheckerboardMaterial>>()
    }
    #[cfg(not(feature = "terrain"))]
    fn deny_terrain_material(self) -> Self {
        self
    }

    // The 3D mesh/material runtime components (bevy_pbr `Mesh3d`/`StandardMaterial`
    // + renzora_shader's `GraphMaterial`/`MaterialResolved`). Stripped with the
    // `render_3d` subsystem — in a 2D-only export bevy_pbr/renzora_shader are gone,
    // so these types don't exist. The serializable mesh/material refs persist
    // instead and rehydrate on load.
    #[cfg(feature = "render_3d")]
    fn deny_render_3d_materials(self) -> Self {
        let b = self
            .deny_component::<Mesh3d>()
            .deny_component::<MeshMaterial3d<StandardMaterial>>();
        // The graph-material half is a separate capability — a game using only
        // StandardMaterial strips `renzora_shader`, and then these two types
        // don't exist to be denied.
        #[cfg(feature = "shader_graph")]
        let b = b
            .deny_component::<MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>()
            .deny_component::<renzora_shader::material::resolver::MaterialResolved>();
        b
    }
    #[cfg(not(feature = "render_3d"))]
    fn deny_render_3d_materials(self) -> Self {
        self
    }
}
