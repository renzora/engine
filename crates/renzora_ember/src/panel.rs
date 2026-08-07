//! `register_panel_content` — removes the per-panel "build once into the active
//! dock leaf" boilerplate.
//!
//! Every native panel used to carry a near-identical `*_content_system`: find the
//! leaf whose active tab is this panel, check a pane isn't already built, then
//! build the content + wrap it in a [`tab_pane`]. That's now a single call:
//!
//! ```ignore
//! app.register_panel_content("my_panel", /* scroll */ true, |commands, fonts| {
//!     let root = commands.spawn(Node { /* … */ }).id();
//!     renzora_ember::reactive::tracked::keyed_list(commands, root, my_snapshot);
//!     root
//! });
//! ```
//!
//! The build closure runs **once** (when the tab is first activated); everything
//! after is driven by the reactive layer. This also registers the id as a native
//! panel so the egui shell's `content_dispatch` skips it.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::dock::{tab_pane, DockLeaf, TabPane};
use crate::font::EmberFonts;

/// Builds a panel's content root (and declares its reactive bindings/lists).
type BuildFn = Box<dyn Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync>;

struct PanelBuilder {
    scroll: bool,
    build: BuildFn,
}

/// Registered native-panel content builders, keyed by panel id.
#[derive(Resource, Default)]
pub struct NativePanelBuilders(HashMap<String, PanelBuilder>);

/// A panel-scoped handle on the `App`, returned by
/// [`RegisterPanelContent::register_panel_content`].
///
/// **Systems added through this are gated on the panel being visible, by
/// default.** That default is the entire point. Panel systems that keep running
/// while their tab is hidden are the largest avoidable cost in the editor's idle
/// frame, and asking every panel author to remember `.run_if(panel_active(id))`
/// demonstrably did not work: surveying the panel crates found some fully gated,
/// several with *zero* gating, and no pattern beyond who happened to think of it.
/// A default you have to opt into is a default that is wrong most of the time.
///
/// Work that genuinely must keep running while the panel is backgrounded — an
/// in-flight load, autosave, teardown that has to observe a despawn — goes
/// through [`PanelScope::always`]. That is deliberately more to type than the
/// gated path, and deliberately reads as a claim at the call site.
pub struct PanelScope<'a> {
    app: &'a mut App,
    id: &'static str,
}

impl<'a> PanelScope<'a> {
    /// Add systems that only run while this panel is the active tab somewhere.
    ///
    /// The gate is [`crate::dock::panel_active`], which covers both the main dock
    /// and torn-off dock windows.
    pub fn systems<M>(
        &mut self,
        schedule: impl bevy::ecs::schedule::ScheduleLabel,
        systems: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
    ) -> &mut Self {
        self.app
            .add_systems(schedule, systems.run_if(crate::dock::panel_active(self.id)));
        self
    }

    /// Add systems that run regardless of whether the panel is visible.
    ///
    /// Only for work that is wrong to pause: background loads, autosave, cleanup
    /// that must observe an event or a despawn while the tab is hidden. If you
    /// are reaching for this because a system "seemed like it should always run",
    /// prefer [`Self::systems`] — a panel that is not on screen has nothing to
    /// show, and pausing is almost always correct.
    pub fn always<M>(
        &mut self,
        schedule: impl bevy::ecs::schedule::ScheduleLabel,
        systems: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
    ) -> &mut Self {
        self.app.add_systems(schedule, systems);
        self
    }

    /// Escape hatch to the underlying `App` for registrations that aren't systems
    /// (resources, assets, plugins, reflection).
    pub fn app(&mut self) -> &mut App {
        self.app
    }
}

/// App extension: register a native panel's content as a single build closure.
pub trait RegisterPanelContent {
    /// Register `id` as a native panel and supply its content builder. `scroll`
    /// wraps the content in a scroll view (`false` if the panel scrolls itself).
    ///
    /// Returns a [`PanelScope`] — add the panel's systems through it and they are
    /// visibility-gated automatically.
    fn register_panel_content<F>(
        &mut self,
        id: &'static str,
        scroll: bool,
        build: F,
    ) -> PanelScope<'_>
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;
}

impl RegisterPanelContent for App {
    fn register_panel_content<F>(
        &mut self,
        id: &'static str,
        scroll: bool,
        build: F,
    ) -> PanelScope<'_>
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    {
        // Tell the egui shell to skip this id (native panels own their content).
        renzora::NativePanelExt::register_native_panel(self, id);
        // Lazily stand up the registry + the single generic build system.
        if !self.world().contains_resource::<NativePanelBuilders>() {
            self.init_resource::<NativePanelBuilders>();
            self.add_systems(Update, build_active_panels);
        }
        self.world_mut()
            .resource_mut::<NativePanelBuilders>()
            .0
            .insert(
                id.to_string(),
                PanelBuilder {
                    scroll,
                    build: Box::new(build),
                },
            );
        PanelScope { app: self, id }
    }
}

/// Build the content for any active leaf whose panel has a registered builder and
/// no pane yet. One system serves every panel that opts into the sugar.
fn build_active_panels(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    builders: Res<NativePanelBuilders>,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        let Some(pb) = builders.0.get(leaf.active.as_str()) else {
            continue;
        };
        let exists = children.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == leaf.active))
        });
        if exists {
            continue;
        }
        let content = (pb.build)(&mut commands, &fonts);
        let pane = tab_pane(&mut commands, leaf.active.as_str(), content, pb.scroll);
        commands.entity(leaf.content).add_child(pane);
    }
}
