//! Editor panels described by plugins, in BSN.
//!
//! A plugin cannot build `bevy_ui` — that is precisely what the C ABI hides — so
//! a panel crosses the boundary as text. It used to cross as a bespoke widget
//! vocabulary (`slider "x" 0 1 -> Type.field`), which worked but meant plugin
//! authors learned a grammar that existed nowhere else and could only ever
//! describe the widgets that grammar knew about.
//!
//! It is now the same BSN a scene uses:
//!
//! ```text
//! Node { flex_direction: Column, row_gap: Px(6.0) }
//! Children [
//!     Text("Widgets"),
//!     EmberDropdown { options: ["Low", "High"], selected: 0 },
//!     EmberTable { headers: ["Name"], rows: [["Cube"]] },
//! ]
//! ```
//!
//! Nothing here parses that — [`renzora_bsn`] does, and it produces **real**
//! `bevy_ui` and `renzora_ember` components rather than wrappers of this
//! module's invention. That is why the vocabulary is open: any component either
//! registry knows about can appear, including the plugin's own, without this
//! file learning its name.
//!
//! Widgets that are builder *functions* reach BSN through the component
//! front-ends in `renzora_ember::widgets::scene` — `EmberDropdown`,
//! `EmberTable`, `EmberTimeline` and friends, each of which builds itself when
//! inserted.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use renzora_ember::panel::RegisterPanelContent;
use renzora_plugin::host::PluginPanels;
use renzora_plugin::sys;

/// One panel's parsed BSN, kept for the builder to spawn from.
struct Registered {
    tree: renzora_bsn::BsnTree,
    action_entry: Option<sys::PanelActionEntry>,
    user: usize,
}

/// Marks a panel's root, so `fill` finds the container to spawn into.
#[derive(Component)]
struct PanelRoot {
    index: usize,
    drawn: bool,
}

#[derive(Resource, Default)]
struct RegisteredPanels(Vec<Registered>);

/// Register every panel plugins asked for.
///
/// Runs from `Plugin::finish`, which is the one hook that has both `&mut App` —
/// `register_panel_content` is an `App` extension — and a guarantee that the
/// plugin loader has already run, since panels do not exist before it does.
pub fn register_plugin_panels(app: &mut App) {
    let panels: Vec<(String, String, String, String, String, Option<sys::PanelActionEntry>, usize)> = {
        let Some(p) = app.world().get_resource::<PluginPanels>() else {
            return;
        };
        p.0.iter()
            .map(|p| {
                (
                    p.id.clone(),
                    p.title.clone(),
                    p.icon.clone(),
                    p.category.clone(),
                    p.markup.clone(),
                    p.on_action,
                    p.user,
                )
            })
            .collect()
    };
    if panels.is_empty() {
        return;
    }

    // Without this the type registry has never heard of it, and a panel naming
    // it in BSN gets "no component called `PanelActionId`" — the component is
    // defined, but defining is not registering.
    app.register_type::<PanelActionId>();
    app.init_resource::<RegisteredPanels>();
    for (id, title, icon, category, source, on_action, user) in panels {
        let tree = match renzora_bsn::bsn_tree::parse(&source) {
            Ok(t) => t,
            Err(e) => {
                error!("[plugin] panel `{id}` has malformed BSN: {e}");
                continue;
            }
        };

        let index = {
            let mut reg = app.world_mut().resource_mut::<RegisteredPanels>();
            reg.0.push(Registered {
                tree,
                action_entry: on_action,
                user,
            });
            reg.0.len() - 1
        };

        {
            let mut reg = app.world_mut().resource_mut::<renzora::ShellPanelRegistry>();
            reg.panels.insert(
                id.clone(),
                renzora::ShellPanelInfo {
                    title,
                    icon: if icon.is_empty() {
                        "puzzle-piece".to_string()
                    } else {
                        icon
                    },
                    category,
                },
            );
        }

        // `register_panel_content` wants a `&'static str` id and a builder that
        // takes only `Commands` and the fonts — no world. Spawning BSN needs the
        // world (both registries live there), so the builder puts down a marked
        // container and a gated system fills it.
        let id: &'static str = Box::leak(id.into_boxed_str());
        app.register_panel_content(id, true, move |commands, _fonts| {
            commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    PanelRoot {
                        index,
                        drawn: false,
                    },
                    Name::new("plugin_panel_root"),
                ))
                .id()
        })
        .systems(Update, fill);

        info!("[plugin] panel `{id}`");
    }

    app.add_systems(Update, dispatch_actions);
}

/// Spawn a panel's BSN the first time it is opened.
fn fill(world: &mut World) {
    let pending: Vec<(Entity, usize)> = world
        .query::<(Entity, &PanelRoot)>()
        .iter(world)
        .filter(|(_, r)| !r.drawn)
        .map(|(e, r)| (e, r.index))
        .collect();
    if pending.is_empty() {
        return;
    }

    for (root, index) in pending {
        let Some(tree) = world
            .get_resource::<RegisteredPanels>()
            .and_then(|p| p.0.get(index))
            .map(|p| p.tree.clone())
        else {
            continue;
        };
        // The same spawner scene loading uses, so a panel gets real components
        // and this module never learns a widget's name.
        renzora_bsn::bsn_tree::spawn_into(world, &tree, root);
        world.entity_mut(root).insert(PanelRoot { index, drawn: true });
    }
}

/// Call the plugin for every action widget that was clicked this frame.
///
/// Bevy's `Interaction` is on the real `Button` entities BSN produced, and the
/// plugin marks the ones it cares about with a `PanelAction` component of its
/// own — so this looks for that pairing rather than for anything this module
/// spawned.
///
/// Exclusive because the handler is handed a command sink, and because the call
/// is `extern "C"` into a `dlopen`'d library: nothing about it should be racing
/// anything else in the schedule.
fn dispatch_actions(world: &mut World) {
    let pressed: Vec<(usize, u32)> = world
        .query::<(&Interaction, &PanelActionId)>()
        .iter(world)
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, a)| (a.panel, a.action))
        .collect();
    if pressed.is_empty() {
        return;
    }

    // Resolved before the sink exists: the sink borrows `Commands`, which
    // borrows the world, so the registry has to be read first.
    let fired: Vec<(sys::PanelActionEntry, usize, u32)> = {
        let Some(reg) = world.get_resource::<RegisteredPanels>() else {
            return;
        };
        pressed
            .into_iter()
            .filter_map(|(panel, action)| {
                let p = reg.0.get(panel)?;
                Some((p.action_entry?, p.user, action))
            })
            .collect()
    };
    if fired.is_empty() {
        return;
    }

    let iface = renzora_plugin::host::interface();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let mut sink = renzora_plugin::host::HostCommandSink::new(&mut commands);
        for (entry, user, action) in fired {
            let name = format!("{action}");
            let payload = sys::PanelAction {
                name: sys::StrRef {
                    ptr: name.as_ptr(),
                    len: name.len(),
                },
                value: 0.0,
                user: user as *mut core::ffi::c_void,
                iface,
                commands: sink.as_ptr(),
            };
            // SAFETY: `entry` came from a `dlopen`'d library the loader keeps
            // alive for the process lifetime, and every pointer above outlives
            // the call. The plugin's thunk carries its own panic guard.
            let status = unsafe { entry(&payload) };
            if status == sys::SystemStatus::Panicked {
                error!("[plugin] panel action {action} panicked");
            }
        }
        sink.drain();
    }
    queue.apply(world);
}

/// Put one of these on a `Button` in a panel's BSN to have clicks reach the
/// plugin's action handler.
///
/// A number rather than a name because a plugin component's fields are the
/// closed set the ABI can describe, and an `i32` is in it while a `String` is
/// not. The plugin matches on the same number it wrote.
#[derive(Component, Reflect, Clone, Copy, Default, Debug)]
#[reflect(Component, Default)]
pub struct PanelActionId {
    pub panel: usize,
    pub action: u32,
}
