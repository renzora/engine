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

use bevy::ecs::component::ComponentId;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::widgets::EmberTextInput;
use renzora_plugin::host::PluginPanels;
use renzora_plugin::sys;

/// One panel's parsed BSN, kept for the builder to spawn from.
struct Registered {
    /// The panel id, which is its identity ACROSS reloads. A slot's index cannot
    /// be — `PanelRoot` and every stamped `PanelActionId` hold that index, so a
    /// reloaded panel has to land back in the same slot, found by id.
    id: String,
    /// The BSN this tree was parsed from, kept solely to answer "did it change?".
    /// Comparing source beats tracking a revision through
    /// `PluginPanels`, which `retire_slot` reorders on every reload.
    source: String,
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
                id: id.clone(),
                source,
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

    app.add_systems(
        Update,
        (
            dispatch_actions,
            apply_bindings,
            // Ordered before the redraw, so markup a plugin set this frame is
            // picked up this frame rather than next. A frame of latency would
            // be invisible most of the time and maddening for anything
            // animated, which is exactly what dynamic panels are for.
            apply_panel_content.before(refresh_reloaded_panels),
            refresh_reloaded_panels,
        ),
    );
}

/// Apply `set_panel_content` calls from plugins.
///
/// Writes into [`PluginPanels`] rather than redrawing here, because the
/// comparison [`refresh_reloaded_panels`] already runs — "does the markup on
/// screen still match what the plugin says?" — is the same question, whether the
/// markup changed because the library was rebuilt or because a system set it.
/// Doing the parse and respawn here as well would be a second implementation of
/// the harder half, free to drift from the one hot reload depends on.
fn apply_panel_content(
    mut parked: ResMut<renzora_plugin::host::PluginServiceCalls>,
    mut panels: ResMut<PluginPanels>,
) {
    use renzora_plugin::panel::{PanelContentHeader, PanelOp};

    let calls = parked.take(renzora_plugin::panel::SERVICE);
    for call in calls {
        let op = PanelOp(call.op);
        if !op.is_known() {
            warn!("[plugin] panel op {} is not one this build has", call.op);
            continue;
        }

        let hdr_len = size_of::<PanelContentHeader>();
        if call.payload.len() < hdr_len {
            warn!("[plugin] panel call sent {} bytes for a header", call.payload.len());
            continue;
        }
        // SAFETY: length checked, and `PanelContentHeader` is `#[repr(C)]` plain
        // data.
        let hdr = unsafe {
            call.payload
                .as_ptr()
                .cast::<PanelContentHeader>()
                .read_unaligned()
        };

        // The length crossed from another compilation unit, so it is untrusted;
        // a bad one would slice past the end. Only the id is length-prefixed —
        // the markup is the remainder — so this is `>` rather than the exact
        // match the HTTP bridge uses.
        let id_end = hdr_len.saturating_add(hdr.id_len as usize);
        if id_end > call.payload.len() {
            warn!(
                "[plugin] panel call claims a {}-byte id but sent {}",
                hdr.id_len,
                call.payload.len() - hdr_len
            );
            continue;
        }

        let id = String::from_utf8_lossy(&call.payload[hdr_len..id_end]).into_owned();
        let markup = String::from_utf8_lossy(&call.payload[id_end..]).into_owned();

        let Some(panel) = panels.0.iter_mut().find(|p| p.id == id) else {
            // Registering a panel needs `&mut App`, so a system cannot create
            // one — this is always a typo or a stale id rather than an ordering
            // problem the caller could fix by waiting.
            warn!("[plugin] set_panel_content for `{id}`, which is not a registered panel");
            continue;
        };
        // Comparing before assigning is what lets a plugin call this every
        // frame: an unchanged string leaves `PluginPanels` untouched, so the
        // redraw below sees no diff and does no work.
        if panel.markup != markup {
            panel.markup = markup;
        }
    }
}

/// Pick up a panel whose plugin has been reloaded, and redraw it.
///
/// `register_plugin_panels` runs once, from `Plugin::finish`, because
/// `register_panel_content` is an `App` extension and a system has no `&mut App`.
/// So a reloaded plugin re-registered its panel into `PluginPanels` and nothing
/// looked again — the panel kept rendering the BSN from the build that had been
/// replaced. This is the part of hot reload that panels were missing.
///
/// Matching is by **id**, and the slot is overwritten in place. A slot's index is
/// held by every live `PanelRoot` and by every `PanelActionId` the host stamped, so
/// a reloaded panel that landed in a new slot would leave both pointing at the old
/// tree — clicks dispatching into a stale thunk.
fn refresh_reloaded_panels(world: &mut World) {
    // What the host currently believes, which a reload has just rewritten.
    let live: Vec<(String, String, Option<sys::PanelActionEntry>, usize, String, String)> = {
        let Some(panels) = world.get_resource::<PluginPanels>() else {
            return;
        };
        panels
            .0
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    p.markup.clone(),
                    p.on_action,
                    p.user,
                    p.title.clone(),
                    p.icon.clone(),
                )
            })
            .collect()
    };
    if live.is_empty() {
        return;
    }

    let mut changed: Vec<usize> = Vec::new();
    let mut retitled: Vec<(String, String, String)> = Vec::new();
    for (id, source, on_action, user, title, icon) in live {
        let Some(mut reg) = world.get_resource_mut::<RegisteredPanels>() else {
            return;
        };
        let Some(index) = reg.0.iter().position(|r| r.id == id) else {
            // A panel id that did not exist at startup. Adding one needs
            // `register_panel_content`, which needs `&mut App` — so this is the one
            // panel change a reload genuinely cannot apply.
            warn_new_panel(&id);
            continue;
        };
        if reg.0[index].source == source {
            continue;
        }
        match renzora_bsn::bsn_tree::parse(&source) {
            Ok(tree) => {
                info!("[plugin] panel `{id}` changed, redrawing");
                let slot = &mut reg.0[index];
                slot.source = source;
                slot.tree = tree;
                // The new build's thunk. Keeping the old one would call into the
                // previous library on the next click.
                slot.action_entry = on_action;
                slot.user = user;
                changed.push(index);
                // The dock holds title and icon separately from the contents, so a
                // renamed panel would otherwise redraw under its old tab label.
                retitled.push((id.clone(), title, icon));
            }
            // Keep the panel that is on screen. A half-edited BSN is a normal
            // intermediate state when someone is typing, and blanking the panel
            // for every keystroke that does not parse would be worse than showing
            // a stale one until it does.
            Err(e) => error!("[plugin] panel `{id}` has malformed BSN, keeping the old one: {e}"),
        }
    }
    if changed.is_empty() {
        return;
    }

    if let Some(mut shell) = world.get_resource_mut::<renzora::ShellPanelRegistry>() {
        for (id, title, icon) in retitled {
            if let Some(info) = shell.panels.get_mut(&id) {
                info.title = title;
                if !icon.is_empty() {
                    info.icon = icon;
                }
            }
        }
    }

    // Clear each affected panel and let `fill` build it again next frame, rather
    // than spawning the new tree here: `fill` already owns that, including the
    // index stamping and the binding hand-off.
    let roots: Vec<Entity> = world
        .query::<(Entity, &PanelRoot)>()
        .iter(world)
        .filter(|(_, r)| changed.contains(&r.index))
        .map(|(e, _)| e)
        .collect();
    for root in roots {
        let index = world.get::<PanelRoot>(root).map(|r| r.index).unwrap_or(0);
        // `try_despawn` on the children: a reactive list may already have removed
        // one, and a plain despawn of a missing entity is a panic.
        if let Ok(mut entity) = world.get_entity_mut(root) {
            entity.despawn_related::<Children>();
        }
        world.entity_mut(root).insert(PanelRoot { index, drawn: false });
    }
}

/// Log an unaddable panel once per id, not once per frame.
fn warn_new_panel(id: &str) {
    use std::sync::Mutex;
    static WARNED: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
    let mut guard = match WARNED.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let seen = guard.get_or_insert_with(Default::default);
    if seen.insert(id.to_string()) {
        warn!(
            "[plugin] panel `{id}` is new — a panel added by a reload cannot be \
             registered with the dock yet. Restart to pick it up."
        );
    }
}

/// Wire up every `bind(Resource.field)` a freshly-spawned panel declared.
///
/// Runs as its own system rather than inside [`fill`] because of a one-frame
/// ordering fact: a widget component's insert hook builds its subtree through the
/// command queue, so at the moment `spawn_into` returns, the entity carrying
/// `Bound<T>` does not exist yet. Binding has to wait for the frame after the
/// spawn, which is exactly what a system polling for `PendingBindings` does. The
/// delay is invisible — `bind_2way` seeds the widget from state on its first run.
fn apply_bindings(world: &mut World) {
    let pending: Vec<(Entity, Vec<renzora_bsn::bsn_tree::BsnBinding>)> = world
        .query::<(Entity, &renzora_bsn::bsn_tree::PendingBindings)>()
        .iter(world)
        .map(|(e, b)| (e, b.0.clone()))
        .collect();
    if pending.is_empty() {
        return;
    }

    for (entity, bindings) in pending {
        // The hook adds exactly one child — the real widget, which is where
        // `Bound<T>` lives. Wait for it rather than binding the wrong entity.
        let Some(widget) = world.get::<Children>(entity).and_then(|c| c.iter().next()) else {
            continue;
        };

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for binding in &bindings {
                match resolve(world, &binding.target) {
                    Ok(field) => bind_field(&mut commands, widget, field),
                    Err(why) => error!(
                        "[plugin] panel binding `{}` on {}.{}: {why}",
                        binding.target, binding.component, binding.field
                    ),
                }
            }
        }
        queue.apply(world);
        world.entity_mut(entity).remove::<renzora_bsn::bsn_tree::PendingBindings>();
    }
}

/// A resolved binding target: where the bytes are and how to read them.
#[derive(Clone, Copy)]
struct BoundField {
    resource: ComponentId,
    offset: usize,
    kind: sys::FieldKind,
}

/// Resolve `Resource.field` against the registered plugin resources.
///
/// Ambiguity is an **error**, not a guess. Two plugins can each register a
/// `Settings`, and picking one would give a panel live control over another
/// plugin's state — the same failure the panel-index bug had. The message names
/// the candidates, and qualifying with the crate (`bind(flock::FlockSettings.x)`)
/// resolves it, since the full type path is matched too.
fn resolve(world: &World, target: &str) -> Result<BoundField, String> {
    let (type_name, field_name) = target
        .rsplit_once('.')
        .ok_or_else(|| "expected `Resource.field`".to_string())?;

    let schemas = world
        .get_resource::<renzora_plugin::host::PluginComponentSchemas>()
        .ok_or_else(|| "no plugin schemas registered".to_string())?;

    let matches: Vec<_> = schemas
        .0
        .iter()
        .filter(|info| info.is_resource)
        .filter(|info| {
            info.type_path == type_name
                || info.type_path.rsplit("::").next() == Some(type_name)
        })
        .collect();

    let info = match matches.as_slice() {
        [] => {
            return Err(format!(
                "no plugin resource named `{type_name}` — a resource must be \
                 `register_resource`d before a panel can bind to it"
            ))
        }
        [one] => one,
        many => {
            let names: Vec<&str> = many.iter().map(|i| i.type_path.as_str()).collect();
            return Err(format!(
                "`{type_name}` is ambiguous between {} — qualify it with the crate",
                names.join(", ")
            ));
        }
    };

    let field = info
        .fields
        .iter()
        .find(|f| f.name == field_name)
        .ok_or_else(|| {
            let known: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
            format!(
                "`{}` has no field `{field_name}` (has: {})",
                info.type_path,
                known.join(", ")
            )
        })?;

    Ok(BoundField {
        resource: info.id,
        offset: field.offset,
        kind: field.kind,
    })
}

/// Two-way-bind the widget's `Bound<T>` to the resource field.
///
/// One `bind_2way` per field kind because `Bound<T>` is generic over the model
/// type: a slider carries `Bound<f32>` and a toggle `Bound<bool>`, so the pair of
/// closures has to be typed. The `I32` case models as `f32` because that is what
/// the numeric widgets carry — the rounding lives in the setter, so the resource
/// still holds an integer.
fn bind_field(commands: &mut Commands, widget: Entity, field: BoundField) {
    let BoundField { resource, offset, kind } = field;
    match kind {
        sys::FieldKind::F32 => renzora_ember::reactive::bind_2way(
            commands,
            widget,
            move |w: &World| super::plugin_resources::read_f32(w, resource, offset),
            move |w: &mut World, v: &f32| {
                super::plugin_resources::write_f32(w, resource, offset, *v)
            },
        ),
        sys::FieldKind::Bool => renzora_ember::reactive::bind_2way(
            commands,
            widget,
            move |w: &World| super::plugin_resources::read_bool(w, resource, offset),
            move |w: &mut World, v: &bool| {
                super::plugin_resources::write_bool(w, resource, offset, *v)
            },
        ),
        sys::FieldKind::I32 => renzora_ember::reactive::bind_2way(
            commands,
            widget,
            move |w: &World| super::plugin_resources::read_i32(w, resource, offset) as f32,
            move |w: &mut World, v: &f32| {
                super::plugin_resources::write_i32(w, resource, offset, v.round() as i32)
            },
        ),
        // Vec3/Quat have no single-value widget to bind, and an unknown kind came
        // from a newer ABI. Neither is worth guessing at.
        other => error!(
            "[plugin] panel binding: field kind {} cannot drive a widget yet",
            other.name()
        ),
    }
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
        stamp_panel_index(world, root, index);
        world.entity_mut(root).insert(PanelRoot { index, drawn: true });
    }
}

/// Point every `PanelActionId` in a freshly-spawned panel at the panel it is
/// actually in.
///
/// A plugin cannot know its own panel index: [`RegisteredPanels`] is one list
/// across every loaded plugin, so the index depends on load order — which depends
/// on what else is in `plugins/`. Writing it plugin-side is guessing. So the
/// plugin supplies only `action`, and the panel is stamped here, where the index
/// is known for certain.
///
/// This is a fix, not a nicety. Every example plugin wrote `panel: 0`, so with
/// more than one panel plugin loaded, all of their clicks dispatched into
/// whichever plugin happened to register first — a button in one panel silently
/// running another plugin's handler.
fn stamp_panel_index(world: &mut World, root: Entity, index: usize) {
    let mut stack = vec![root];
    let mut found = Vec::new();
    while let Some(entity) = stack.pop() {
        if world.get::<PanelActionId>(entity).is_some() {
            found.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    for entity in found {
        if let Some(mut action) = world.get_mut::<PanelActionId>(entity) {
            action.panel = index;
        }
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
    let pressed: Vec<(usize, u32, String)> = world
        .query::<(&Interaction, &PanelActionId, Option<&Children>)>()
        .iter(world)
        .filter(|(i, _, _)| **i == Interaction::Pressed)
        .map(|(_, a, kids)| {
            let kids: Vec<Entity> = kids.map(|k| k.iter().collect()).unwrap_or_default();
            (a.panel, a.action, kids)
        })
        .collect::<Vec<_>>()
        .into_iter()
        // `EmberInput` builds its `EmberTextInput` as a CHILD, so a widget's
        // text is one level down from the entity carrying the action id.
        .map(|(panel, action, kids)| {
            let text = kids
                .into_iter()
                .find_map(|c| world.get::<EmberTextInput>(c).map(|t| t.value.clone()))
                .unwrap_or_default();
            (panel, action, text)
        })
        .collect();
    if pressed.is_empty() {
        return;
    }

    // Resolved before the sink exists: the sink borrows `Commands`, which
    // borrows the world, so the registry has to be read first.
    let fired: Vec<(sys::PanelActionEntry, usize, u32, String)> = {
        let Some(reg) = world.get_resource::<RegisteredPanels>() else {
            return;
        };
        pressed
            .into_iter()
            .filter_map(|(panel, action, text)| {
                let p = reg.0.get(panel)?;
                Some((p.action_entry?, p.user, action, text))
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
        for (entry, user, action, text) in fired {
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
                // Borrowed, not copied: `text` outlives the call below, and the
                // guest's `Action::text` hands back a `&str` so a plugin that
                // wants to keep it has to say so.
                text: sys::StrRef {
                    ptr: text.as_ptr(),
                    len: text.len(),
                },
            };
            // SAFETY: `entry` came from a `dlopen`'d library the loader keeps
            // alive for the process lifetime, and every pointer above outlives
            // the call. The plugin's thunk carries its own panic guard.
            let status = unsafe { entry(&payload) };
            // An unrecognised status is a failure, not a success — see the
            // dispatcher in `renzora_plugin::host`.
            if status == sys::SystemStatus::Panicked || !status.is_known() {
                error!("[plugin] panel action {action} panicked");
            }
        }
        sink.drain();
    }
    queue.apply(world);
}

/// Put one of these on a `Button` in a panel's BSN to have clicks reach the
/// plugin's action handler: `PanelActionId { action: 1 }`.
#[derive(Component, Reflect, Clone, Copy, Default, Debug)]
#[reflect(Component, Default)]
pub struct PanelActionId {
    /// Which panel this widget belongs to. **Host-assigned — a plugin should
    /// leave it at the default.** It indexes [`RegisteredPanels`], which is one
    /// list across every loaded plugin, so the correct value depends on load
    /// order and only the host knows it. [`stamp_panel_index`] overwrites
    /// whatever was in the BSN.
    pub panel: usize,
    /// Which action fired, chosen by the plugin. A number rather than a name
    /// because a plugin component's fields are the closed set the ABI can
    /// describe, and an `i32` is in it while a `String` is not. The plugin
    /// matches on the same number it wrote — it arrives as
    /// `Action::name()`, stringified.
    pub action: u32,
}
