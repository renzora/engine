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
use renzora_ember::settings_sections::RegisterSettingsSection;
use renzora_ember::reactive::{Bound, Rx};
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
    #[allow(clippy::type_complexity)]
    let panels: Vec<(
        String,
        String,
        String,
        String,
        String,
        Option<sys::PanelActionEntry>,
        usize,
        bool,
    )> = {
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
                    p.settings,
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
    // See the `fill` note inside the loop.
    let mut settings_fill_added = false;
    app.register_type::<PanelActionId>();
    // A plugin's markup has to be able to name a clickable thing, and dispatch
    // needs `Interaction` and the action id on ONE entity. `Button` requires
    // Node + Interaction + FocusPolicy, but a reflected spawn does not apply
    // required components — so a panel should still name Node and Interaction
    // itself, and these registrations are what make all three resolvable at all.
    // Without them BSN logs "no component called `Button`" and silently drops it,
    // which is a button that renders as bare text and never fires.
    app.register_type::<Button>();
    app.register_type::<Interaction>();
    // So a panel can size its own text. Everything defaults to the base UI size
    // otherwise, which is far too large for a dense panel.
    app.register_type::<TextFont>();
    app.init_resource::<RegisteredPanels>();
    for (id, title, icon, category, source, on_action, user, is_settings) in panels {
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

        // A settings section takes the same slot — dispatch and
        // `set_panel_content` both resolve through `RegisteredPanels`, so it has
        // to be there — but registers into the Settings overlay instead of the
        // dock. It gets no `ShellPanelInfo`: it has no tab to name and no layout
        // entry to persist.
        if is_settings {
            let id_owned: &'static str = Box::leak(id.clone().into_boxed_str());
            let icon = if icon.is_empty() {
                "puzzle-piece".to_string()
            } else {
                icon
            };
            app.register_settings_section(id_owned, &title, &icon, move |commands, _fonts| {
                commands
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            width: Val::Percent(100.0),
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        PanelRoot {
                            index,
                            drawn: false,
                        },
                        Name::new("plugin_settings_root"),
                    ))
                    .id()
            });
            // `fill` is scoped to a panel when `register_panel_content` adds it,
            // and a settings section has no panel to scope it to — so it is added
            // once, globally, for them. It is gated on `drawn`, so a second
            // registration would be idle rather than wrong; `is_settings` firing
            // for several sections is why this needs to be idempotent at all.
            if !std::mem::replace(&mut settings_fill_added, true) {
                // panel-systems-ungated: a settings SECTION has no panel to
                // scope to — that is the whole difference from the branch
                // below, and it is why this is added globally instead.
                app.add_systems(Update, fill);
            }
            info!("[plugin] settings section `{id}`");
            continue;
        }

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

    // panel-systems-ungated: these dispatch for EVERY plugin panel and every
    // settings section at once, so there is no single panel whose visibility
    // could gate them. Scoping them to one would stop the others updating.
    app.add_systems(
        Update,
        (
            dispatch_actions,
            dispatch_input_changes,
            dispatch_value_changes,
            apply_bindings,
        ),
    );
    // `PostUpdate`, not `Update`, and this is load-bearing rather than tidy.
    //
    // A plugin's `set_panel_content` is a DEFERRED command: it reaches
    // `PluginServiceCalls` only when `Update`'s command queue flushes, which is
    // after every `Update` system has run. Sitting in `Update` alongside the
    // plugin dispatcher, this claimed the queue *before* the call was in it, and
    // then `discard_unhandled_service_calls` — the `Last` sweep that stops a
    // build with no bridge growing the queue forever — binned it before the next
    // frame could look. Every single call was queued and swept, which presents
    // as a panel that renders once and never updates again, with nothing logged
    // anywhere.
    //
    // Running here puts it after the flush and before the sweep, so a call made
    // during `Update` is claimed in the same frame it was made.
    // panel-systems-ungated: this CLAIMS queued `set_panel_content` calls, and a
    // call that is not claimed in the frame it was made is binned by the `Last`
    // sweep (see the note above). Gating it on visibility would silently discard
    // every update a plugin makes to a panel while that panel is hidden — so the
    // panel would be stale on the frame it reappeared, with nothing logged.
    app.add_systems(
        PostUpdate,
        (
            apply_panel_content.before(refresh_reloaded_panels),
            refresh_reloaded_panels,
        ),
    );
}

/// Fire a panel action when a text input's contents change.
///
/// [`dispatch_actions`] fires on `Interaction::Pressed`, which a text input
/// never receives while someone is typing in it — so without this the `text`
/// field added in MINOR 4.6 would only ever reach a plugin when a *button* was
/// clicked, and a button has no input child to read from.
///
/// So the input reports itself. A plugin puts a `PanelActionId` on the
/// `EmberInput` and gets an action on every keystroke carrying the current
/// value; it caches that, and reads the cache when its Send button fires. State
/// lives in the plugin, which is where the rest of it already lives.
///
/// `EmberInput` builds its `EmberTextInput` as a CHILD, so the changed component
/// is one level below the entity carrying the action id — hence the walk up
/// through `ChildOf` rather than a single query.
fn dispatch_input_changes(world: &mut World) {
    let changed: Vec<(usize, u32, String)> = world
        .query_filtered::<(&EmberTextInput, &ChildOf), Changed<EmberTextInput>>()
        .iter(world)
        .map(|(input, parent)| (parent.parent(), input.value.clone()))
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|(parent, value)| {
            let id = world.get::<PanelActionId>(parent)?;
            Some((id.panel, id.action, value))
        })
        .collect();
    if changed.is_empty() {
        return;
    }
    dispatch(world, changed);
}

/// Fire a panel action when a non-text widget's value changes.
///
/// The companion to [`dispatch_input_changes`], and it exists because a
/// dropdown, toggle, slider or checkbox reports through neither of the other two
/// paths: the entity carrying the `PanelActionId` never registers a press (the
/// clickable part is a child), and it holds no `EmberTextInput` either. Without
/// this a plugin can *render* a dropdown and never learn that it was used —
/// which is exactly how it presented: settings that looked live and changed
/// nothing.
///
/// `Bound<T>` is ember's own "this widget's value" component, so watching it
/// covers every widget built on the same binding rather than any one kind.
///
/// **All three instantiations, not just `Bound<usize>`.** `Bound` is generic
/// over the model type and each instantiation is a distinct component, so a
/// query for one sees nothing of the others. Watching only `usize` was the
/// original form of this function, and it fixed dropdowns while leaving the
/// defect it describes fully intact for the widgets beside them: `EmberToggle`
/// and `EmberCheckbox` carry `Bound<bool>`, `EmberSliderWidget` carries
/// `Bound<f32>`, and all three rendered, animated on click, and reported
/// nothing. A plugin could ship a settings toggle that did precisely as much as
/// a painted one.
///
/// The value goes out in `value`, not `text`: text is for widgets holding a
/// string. `bool` crosses as 0.0 or 1.0 — the ABI's action payload is one `f32`
/// and adding a kind tag to distinguish "false" from "index 0" would mean a
/// boundary change for something every caller already knows, since a plugin
/// knows which widget it put behind that action id.
fn dispatch_value_changes(world: &mut World) {
    let mut changed: Vec<(Entity, f32)> = Vec::new();
    changed.extend(
        world
            .query_filtered::<(&Bound<usize>, &ChildOf), Changed<Bound<usize>>>()
            .iter(world)
            .map(|(bound, parent)| (parent.parent(), bound.0 as f32)),
    );
    changed.extend(
        world
            .query_filtered::<(&Bound<bool>, &ChildOf), Changed<Bound<bool>>>()
            .iter(world)
            .map(|(bound, parent)| (parent.parent(), if bound.0 { 1.0 } else { 0.0 })),
    );
    changed.extend(
        world
            .query_filtered::<(&Bound<f32>, &ChildOf), Changed<Bound<f32>>>()
            .iter(world)
            .map(|(bound, parent)| (parent.parent(), bound.0)),
    );

    let changed: Vec<(usize, u32, f32)> = changed
        .into_iter()
        .filter_map(|(parent, value)| {
            let id = world.get::<PanelActionId>(parent)?;
            Some((id.panel, id.action, value))
        })
        .collect();
    if changed.is_empty() {
        return;
    }
    // Same tail as the other two, with the value in place of the text.
    dispatch_valued(world, changed);
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
            // `debug`, not `info`: a plugin driving a live panel sends this every
            // frame it changes something, which for a text field is every keystroke.
            debug!("[plugin] set_panel_content applied to `{id}` ({} bytes)", panel.markup.len());
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
                debug!("[plugin] panel `{id}` changed, redrawing (slot {index})");
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
                match resolve(&Rx::new(&*world), &binding.target) {
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
fn resolve(world: &Rx, target: &str) -> Result<BoundField, String> {
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
        sys::FieldKind::F32 => renzora_ember::reactive::tracked::bind_2way(
            commands,
            widget,
            move |rx: &Rx| super::plugin_resources::tracked_read(rx, resource, offset, super::plugin_resources::read_f32),
            move |w: &mut World, v: &f32| {
                super::plugin_resources::write_f32(w, resource, offset, *v)
            },
        ),
        sys::FieldKind::Bool => renzora_ember::reactive::tracked::bind_2way(
            commands,
            widget,
            move |rx: &Rx| super::plugin_resources::tracked_read(rx, resource, offset, super::plugin_resources::read_bool),
            move |w: &mut World, v: &bool| {
                super::plugin_resources::write_bool(w, resource, offset, *v)
            },
        ),
        sys::FieldKind::I32 => renzora_ember::reactive::tracked::bind_2way(
            commands,
            widget,
            move |rx: &Rx| super::plugin_resources::tracked_read(rx, resource, offset, super::plugin_resources::read_i32) as f32,
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

    dispatch(world, pressed);
}

/// Call each plugin's action thunk. Shared by [`dispatch_actions`] and
/// [`dispatch_input_changes`] so the two cannot drift — the unsafe call, the
/// panic-status handling and the command drain are the same either way, and only
/// the question of what counts as "fired" differs.
fn dispatch(world: &mut World, fired_raw: Vec<(usize, u32, String)>) {
    dispatch_inner(
        world,
        fired_raw
            .into_iter()
            .map(|(p, a, t)| (p, a, t, 0.0))
            .collect(),
    )
}

/// [`dispatch`] for widgets that report a number rather than a string.
fn dispatch_valued(world: &mut World, fired_raw: Vec<(usize, u32, f32)>) {
    dispatch_inner(
        world,
        fired_raw
            .into_iter()
            .map(|(p, a, v)| (p, a, String::new(), v))
            .collect(),
    )
}

#[allow(clippy::type_complexity)]
fn dispatch_inner(world: &mut World, fired_raw: Vec<(usize, u32, String, f32)>) {
    let pressed = fired_raw;
    // Resolved before the sink exists: the sink borrows `Commands`, which
    // borrows the world, so the registry has to be read first.
    #[allow(clippy::type_complexity)]
    let fired: Vec<(sys::PanelActionEntry, usize, u32, String, f32)> = {
        let Some(reg) = world.get_resource::<RegisteredPanels>() else {
            return;
        };
        pressed
            .into_iter()
            .filter_map(|(panel, action, text, value)| {
                let p = reg.0.get(panel)?;
                Some((p.action_entry?, p.user, action, text, value))
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
        for (entry, user, action, text, value) in fired {
            let name = format!("{action}");
            let payload = sys::PanelAction {
                name: sys::StrRef {
                    ptr: name.as_ptr(),
                    len: name.len(),
                },
                value,
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
