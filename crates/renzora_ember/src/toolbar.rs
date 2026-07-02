//! Panel toolbar registry — a shared toolbar strip (rendered by the editor shell
//! just below the document tabs) whose contents follow the **active panels**.
//!
//! A panel (or a plugin) registers toolbar items keyed by the panel's dock id
//! (e.g. `"viewport"`, `"code_editor"`, `"material_graph"`). Each item is a
//! *builder closure* that receives `&mut Commands` + [`EmberFonts`], so it can
//! spawn ANY ember widget — buttons, dropdowns, sliders, inputs, checkboxes,
//! toggle switches, color pickers — and wire reactivity with the `bind_*`
//! helpers (e.g. read `EditorSelection` to react when a mesh is picked).
//!
//! The host shows an item's group only while its panel is the active (visible)
//! tab in its dock leaf ([`Dock::tree`] + `is_active_tab`), so the strip swaps
//! automatically as you move between panels / workspaces. Nothing here is keyed
//! to a *workspace* — purely to which panels are on screen.

use bevy::prelude::*;
use std::sync::Arc;

use crate::dock::Dock;
use crate::font::{icon_text, EmberFonts};
use crate::reactive::{bind_bg, bind_display};
use crate::theme::{panel_bg, rgb, text_primary};

/// Height of the toolbar strip (matches the viewport header so the Scene
/// toolbar — the viewport header — lines up with everyone else's).
pub const TOOLBAR_HEIGHT: f32 = 28.0;

/// Builds one toolbar item (a button, a dropdown, a labelled slider, a whole
/// sub-group of widgets…) and returns its root entity. Gets full `Commands` +
/// fonts, so it can use any ember widget and any reactive binding.
pub type ToolbarBuilder = Arc<dyn Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync>;

/// One registered toolbar item, scoped to the panel it belongs to.
#[derive(Clone)]
pub struct PanelToolbarItem {
    /// Dock panel id this item belongs to (e.g. `"code_editor"`). The item's
    /// group shows while this — or any [`also_visible_for`] — panel is the
    /// active/visible tab in its leaf. Also the group key for layout.
    pub panel: &'static str,
    /// Extra panel ids that also make this item's group visible (e.g. the
    /// secondary viewport slots `viewport-2/3/4` for the main viewport toolbar).
    pub also_visible_for: Vec<&'static str>,
    /// Sort order within the panel's group (lower = earlier).
    pub order: i32,
    pub build: ToolbarBuilder,
}

/// Registry of panel toolbar items. Populated at plugin-build time; consumed
/// once by the shell when it builds the toolbar strip.
#[derive(Resource, Default, Clone)]
pub struct PanelToolbars {
    items: Vec<PanelToolbarItem>,
}

impl PanelToolbars {
    pub fn register(&mut self, item: PanelToolbarItem) {
        self.items.push(item);
    }

    /// Distinct panel ids that have at least one item, in first-seen order.
    pub fn panels(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for it in &self.items {
            if !out.contains(&it.panel) {
                out.push(it.panel);
            }
        }
        out
    }

    /// A panel's items, sorted by `order`.
    pub fn for_panel(&self, panel: &str) -> Vec<PanelToolbarItem> {
        let mut v: Vec<PanelToolbarItem> =
            self.items.iter().filter(|i| i.panel == panel).cloned().collect();
        v.sort_by_key(|i| i.order);
        v
    }

    /// All panel ids whose being-active should show `panel`'s group — the group
    /// key plus every `also_visible_for` from its items (deduped).
    pub fn visible_panels_for(&self, panel: &str) -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = Vec::new();
        for it in self.items.iter().filter(|i| i.panel == panel) {
            if !ids.contains(&it.panel) {
                ids.push(it.panel);
            }
            for extra in &it.also_visible_for {
                if !ids.contains(extra) {
                    ids.push(*extra);
                }
            }
        }
        ids
    }
}

/// A one-shot action carried by a convenience toolbar button. Run by
/// [`run_toolbar_button_actions`] when the button is clicked.
#[derive(Component, Clone)]
pub struct ToolbarButtonAction(pub Arc<dyn Fn(&mut World) + Send + Sync>);

/// `App` extension: the developer-facing API for adding toolbar items/buttons.
pub trait PanelToolbarExt {
    /// Register a fully custom toolbar item for `panel`. The closure may spawn
    /// any ember widget(s) and return the item's root entity.
    fn register_panel_toolbar<F>(&mut self, panel: &'static str, build: F) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;

    /// Like [`register_panel_toolbar`] but with an explicit sort `order`.
    fn register_panel_toolbar_ordered<F>(
        &mut self,
        panel: &'static str,
        order: i32,
        build: F,
    ) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;

    /// Like [`register_panel_toolbar`] but the item's group is shown while ANY
    /// of `panels` is the active tab (e.g. the main viewport toolbar showing for
    /// `viewport` and the secondary slots `viewport-2/3/4`). `panels[0]` is the
    /// group key; the rest are extra visibility triggers. Empty slice is a no-op.
    fn register_panel_toolbar_multi<F>(&mut self, panels: &[&'static str], build: F) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static;

    /// Convenience: register a simple icon button for `panel`. `icon` is a
    /// kebab-case Phosphor name; `on_click` runs (deferred) when it's pressed.
    fn register_panel_toolbar_button<F>(
        &mut self,
        panel: &'static str,
        icon: &'static str,
        tooltip: &'static str,
        on_click: F,
    ) -> &mut Self
    where
        F: Fn(&mut World) + Send + Sync + 'static;
}

impl PanelToolbarExt for App {
    fn register_panel_toolbar<F>(&mut self, panel: &'static str, build: F) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    {
        self.register_panel_toolbar_ordered(panel, 0, build)
    }

    fn register_panel_toolbar_ordered<F>(
        &mut self,
        panel: &'static str,
        order: i32,
        build: F,
    ) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    {
        self.init_resource::<PanelToolbars>();
        self.world_mut()
            .resource_mut::<PanelToolbars>()
            .register(PanelToolbarItem {
                panel,
                also_visible_for: Vec::new(),
                order,
                build: Arc::new(build),
            });
        self
    }

    fn register_panel_toolbar_multi<F>(&mut self, panels: &[&'static str], build: F) -> &mut Self
    where
        F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    {
        let Some((&panel, rest)) = panels.split_first() else {
            return self;
        };
        self.init_resource::<PanelToolbars>();
        self.world_mut()
            .resource_mut::<PanelToolbars>()
            .register(PanelToolbarItem {
                panel,
                also_visible_for: rest.to_vec(),
                order: 0,
                build: Arc::new(build),
            });
        self
    }

    fn register_panel_toolbar_button<F>(
        &mut self,
        panel: &'static str,
        icon: &'static str,
        tooltip: &'static str,
        on_click: F,
    ) -> &mut Self
    where
        F: Fn(&mut World) + Send + Sync + 'static,
    {
        let icon = icon.to_string();
        let tooltip = tooltip.to_string();
        let action: Arc<dyn Fn(&mut World) + Send + Sync> = Arc::new(on_click);
        self.register_panel_toolbar(panel, move |commands, fonts| {
            toolbar_button(commands, fonts, &icon, &tooltip, action.clone())
        })
    }
}

/// A standard 26×22 icon button used by [`register_panel_toolbar_button`].
fn toolbar_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    tooltip: &str,
    action: Arc<dyn Fn(&mut World) + Send + Sync>,
) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 14.0);
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ToolbarButtonAction(action),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            crate::widgets::HoverTooltip::new(tooltip),
            Name::new(format!("toolbar-btn:{tooltip}")),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    // Hover wash via the live theme's hovered-bg token.
    bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
            rgb(crate::theme::hover_bg())
        }
        _ => Color::NONE,
    });
    btn
}

/// Run a convenience toolbar button's action when it's pressed (deferred, so
/// the action gets exclusive `&mut World`).
fn run_toolbar_button_actions(
    mut commands: Commands,
    q: Query<(&Interaction, &ToolbarButtonAction), Changed<Interaction>>,
) {
    for (interaction, action) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let action = action.0.clone();
        commands.queue(move |w: &mut World| (action)(w));
    }
}

/// Build the toolbar strip the shell mounts below the document tabs. One
/// horizontal row; each registered panel gets a group whose display is bound to
/// "this panel is the active dock tab", so only the on-screen panels' toolbars
/// show (and they concatenate if several are visible at once).
pub fn build_toolbar_host(
    commands: &mut Commands,
    fonts: &EmberFonts,
    registry: &PanelToolbars,
) -> Entity {
    let host = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                // Center the whole toolbar; visible panels' groups append into
                // this one centered cluster (in registration order).
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("panel-toolbar-host"),
        ))
        .id();

    for panel in registry.panels() {
        let group = commands
            .spawn((
                Node {
                    // Content-sized (no flex-grow) so groups sit next to each
                    // other and the host can center the combined cluster.
                    height: Val::Px(TOOLBAR_HEIGHT),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
                Name::new(format!("toolbar-group:{panel}")),
            ))
            .id();
        let kids: Vec<Entity> = registry
            .for_panel(panel)
            .iter()
            .map(|it| (it.build)(commands, fonts))
            .collect();
        commands.entity(group).add_children(&kids);
        // Show this group while ANY of its visibility panels is the active
        // (visible) dock tab — for the viewport, that's any of the 4 slots.
        let ids = registry.visible_panels_for(panel);
        bind_display(commands, group, move |w| {
            w.get_resource::<Dock>()
                .is_some_and(|d| ids.iter().any(|id| d.tree.is_active_tab(id)))
        });
        commands.entity(host).add_child(group);
    }
    // Hide the whole toolbar strip during play mode so the running game gets a
    // clean view (no editor toolbars). The per-group binds above still apply when
    // not playing.
    bind_display(commands, host, |w| {
        !w.get_resource::<renzora::core::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false)
    });
    host
}

/// Registers the convenience-button click driver. Called from [`crate::EmberPlugin`].
pub(crate) fn register(app: &mut App) {
    app.init_resource::<PanelToolbars>();
    app.add_systems(Update, run_toolbar_button_actions);
}
