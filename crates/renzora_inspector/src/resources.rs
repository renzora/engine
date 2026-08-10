//! The Resources panel — the world's reflected ECS resources, browsable and
//! editable.
//!
//! The inspector answers "what is on the thing I selected". A resource has no
//! entity to select in the scene (see [`crate::plugin_resources`] for the same
//! argument made about plugin globals), so the state that is *not* attached to
//! anything — `Time`, `EditorSettings`, every plugin's config, every one of the
//! engine's own globals — had nowhere to be looked at. This panel is that place.
//!
//! Its neighbour [`crate::plugin_resources`] deliberately stays: that one draws
//! the resources a C-ABI plugin declared through its field schema, which are not
//! Rust types this build knows and so cannot be reflected at all. This one draws
//! what `bevy_reflect` can see. The two cover disjoint sets.
//!
//! ## Master/detail, because a world has hundreds of resources
//!
//! The list is a virtualized flat list of names; the fields belong to whichever
//! row is selected, and **only** that one. Inline expandable sections were the
//! obvious first shape and the wrong one twice over: variable-height bodies
//! fight `renzora_ember::virtual_scroll`'s row measurement, and several expanded
//! resources at once means several sets of live two-way bindings for state
//! nobody is comparing. One selection at a time means the reflection walk
//! happens on click, and the only bindings alive are the ones on screen.
//!
//! ## Unreflected resources are counted, not listed
//!
//! A resource without `#[reflect(Resource)]` has no name this build can show:
//! naming a component outside the type registry means `ComponentInfo::name()`,
//! which returns `"<Enable the debug feature to see the name>"` unless Bevy's
//! `debug` feature is compiled in — and this workspace does not enable it. Rows
//! for those would be identically-named and openable to nothing, so the toolbar
//! reports how many exist instead, and the count never reads as "this is
//! everything".
//!
//! ## Why a resource needs no reflection code of its own
//!
//! Bevy 0.19 made `Resource: Component`: a resource's value lives as a component
//! on a hidden entity. So [`crate::reflect_source`]'s component reader and writer
//! work on a resource unchanged, given that entity — which is what
//! [`reflect_source::world_resources`] hands back. There is no second reflection
//! path here to drift from the inspector's.
//!
//! ## One scan, in one place
//!
//! Naming every resource means resolving several hundred `ComponentId`s through
//! the type registry and allocating a display string for each — too much to do
//! per frame, and too much to do twice. [`refresh_index`] does it once into
//! [`ResourceIndex`] and bumps a generation only when the result actually
//! changed; the list, the count and the detail pane are all reactive readers of
//! that resource, so they re-run when it moves and not otherwise.
//!
//! ## Edits are not undoable, on purpose
//!
//! Every other reflected field in the editor records a `renzora_undo` step. These
//! do not: undo stacks here are per-document, and the active one is almost always
//! the scene's. Poking a global into that stack would mean Ctrl+Z in the viewport
//! silently reverting a debug tweak made in a different panel — the two are not
//! the same edit history, and pretending they are is worse than having none.

use std::hash::{Hash, Hasher};

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_editor_framework::{FieldType, FieldValue};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::{fill_control, inspector_row, inspector_stripe};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_text, keyed_list_tokened};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{
    border, panel_bg, rgb, selection, text_muted, text_primary, value_text,
};
use renzora_ember::virtual_scroll::virtual_scroll_versioned;
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown, scroll_view, text_input, toggle_switch, DragRange,
    DragSnap, EmberTextInput,
};

use crate::reflect_source::{self, ReflectField};

pub const PANEL_ID: &str = "resources";

/// Rows built above and below the viewport, so a fast scroll doesn't flash
/// empty. Matches what the hierarchy uses.
const OVERSCAN: usize = 6;

// ── state ────────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
struct ResourceBrowser {
    /// The search box's text, mirrored here so the index pass can read it
    /// without querying the widget.
    filter: String,
    /// The selected row's type path. Kept as the type rather than an `Entity`
    /// so a selection survives a resource being removed and re-inserted (which
    /// gives it a new hidden entity but the same type).
    selected: Option<&'static str>,
    /// The filter changed, so the next [`refresh_index`] must not wait for its
    /// polling tick.
    refresh: bool,
}

/// The named, filtered, sorted resource list — the single source both the list
/// and the detail pane read.
#[derive(Resource, Default)]
struct ResourceIndex {
    rows: Vec<IndexRow>,
    /// Bumped only when [`rows`](Self::rows) actually changed, so a reader gated
    /// on it re-runs no more often than it must.
    generation: u64,
    /// How many reflected resources exist before the filter, for the toolbar
    /// count.
    total: usize,
    /// How many resources this build cannot name or read at all — reported so
    /// the count doesn't imply the list is everything. See
    /// [`reflect_source::WorldResources::unreflected`].
    unreflected: usize,
}

struct IndexRow {
    /// Identity for selection and for the generation hash.
    type_path: &'static str,
    /// Type name with every module path stripped.
    display: String,
    /// Module path, shown muted after the name.
    module: String,
    /// The entity Bevy stores this resource's value on.
    entity: Entity,
    cid: ComponentId,
}

/// The panel's chrome, marked so the systems can find each piece.
#[derive(Component)]
struct ResourceSearchInput;

/// A list row, carrying the type path a click selects.
#[derive(Component)]
struct ResourceRow(&'static str);

// ── naming ───────────────────────────────────────────────────────────────────

fn last_segment(s: &str) -> &str {
    s.rsplit("::").next().unwrap_or(s)
}

/// `bevy_time::time::Time<bevy_time::virt::Virtual>` → `Time<Virtual>`.
///
/// Not [`reflect_source`]'s `short_name`, which drops everything from the first
/// `<` — that would show `Time` three times over, for three different resources.
/// The generic arguments are the only thing telling them apart.
fn short_type_name(full: &str) -> String {
    const DELIMS: [char; 8] = ['<', '>', ',', ' ', '(', ')', '[', ']'];
    let mut out = String::with_capacity(full.len());
    let mut seg_start = 0usize;
    for (i, ch) in full.char_indices() {
        if DELIMS.contains(&ch) {
            out.push_str(last_segment(&full[seg_start..i]));
            out.push(ch);
            seg_start = i + ch.len_utf8();
        }
    }
    out.push_str(last_segment(&full[seg_start..]));
    out
}

/// The module path a type came from, for the muted right-hand side of a row.
fn module_path(full: &str) -> String {
    let base = full.split('<').next().unwrap_or(full);
    match base.rfind("::") {
        Some(i) => base[..i].to_string(),
        None => String::new(),
    }
}

fn hash_of(f: impl FnOnce(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f(&mut h);
    h.finish()
}

fn key_hash(key: &str) -> u64 {
    hash_of(|h| key.hash(h))
}

// ── reading and writing a field ──────────────────────────────────────────────

/// Read one field while declaring what it depends on.
///
/// `read_field` takes a plain `&World`, so the binding around it would otherwise
/// have to give up on tracking and re-run every frame. Naming the resource's
/// component lets it declare the dependency instead — the same trade the
/// inspector's rows make, and the reason an open resource costs nothing while
/// its values are not changing.
fn tracked_read(
    rx: &Rx,
    section: (Entity, ComponentId),
    type_path: &'static str,
    path: &'static str,
    read_only: bool,
) -> Option<FieldValue> {
    let (resource, cid) = section;
    rx.track_component_id(resource, cid);
    reflect_source::read_field(rx.manually_tracked(), resource, type_path, path, read_only)
}

fn write(
    resource: Entity,
    type_path: &'static str,
    path: &'static str,
) -> impl Fn(&mut World, FieldValue) + Send + Sync + 'static {
    move |world: &mut World, value: FieldValue| {
        reflect_source::write_field(world, resource, type_path, path, value);
    }
}

fn format_value(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::ReadOnly(s)) | Some(FieldValue::String(s)) => s.clone(),
        Some(FieldValue::Enum(s)) => s.clone(),
        Some(FieldValue::Float(f)) => format!("{f}"),
        Some(FieldValue::Bool(b)) => format!("{b}"),
        Some(FieldValue::Vec3(a)) => format!("({:.3}, {:.3}, {:.3})", a[0], a[1], a[2]),
        Some(FieldValue::Color(c)) => format!("({:.3}, {:.3}, {:.3})", c[0], c[1], c[2]),
        Some(FieldValue::ColorRgba(c)) => {
            format!("({:.3}, {:.3}, {:.3}, {:.3})", c[0], c[1], c[2], c[3])
        }
        Some(FieldValue::Asset(s)) => s.clone().unwrap_or_else(|| "—".to_string()),
        None => "—".to_string(),
    }
}

// ── field widgets ────────────────────────────────────────────────────────────

/// The editing widget for one field, two-way bound through reflection.
fn build_control(
    commands: &mut Commands,
    fonts: &EmberFonts,
    section: (Entity, ComponentId),
    type_path: &'static str,
    field: &ReflectField,
) -> Entity {
    let path = field.path;
    match field.field_type {
        FieldType::Float { speed, min, max } => {
            let init = match field.value {
                FieldValue::Float(v) => v,
                _ => 0.0,
            };
            let dv = drag_value(commands, &fonts.ui, "", value_text(), init, speed.max(0.001));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let set = write(section.0, type_path, path);
            bind_2way(
                commands,
                dv,
                move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w: &mut World, v: &f32| set(w, FieldValue::Float(*v)),
            );
            fill_control(commands, dv);
            dv
        }
        FieldType::Int { min, max } => {
            let init = match field.value {
                FieldValue::Float(v) => v,
                _ => 0.0,
            };
            // Quarter-unit-per-pixel scrub snapped to whole steps, matching the
            // inspector — the snap is what stops the rounded write-back from
            // fighting the drag.
            let dv = drag_value(commands, &fonts.ui, "", value_text(), init, 0.25);
            commands.entity(dv).insert(DragSnap(1.0));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let set = write(section.0, type_path, path);
            bind_2way(
                commands,
                dv,
                move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w: &mut World, v: &f32| set(w, FieldValue::Float(*v)),
            );
            fill_control(commands, dv);
            dv
        }
        FieldType::Vec3 { speed } => {
            let init = match field.value {
                FieldValue::Vec3(a) => a,
                _ => [0.0; 3],
            };
            const AXES: [(&str, (u8, u8, u8)); 3] = [
                ("X", (230, 90, 90)),
                ("Y", (130, 200, 90)),
                ("Z", (90, 150, 230)),
            ];
            let wrap = commands
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_grow: 1.0,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .id();
            for (i, (axis, color)) in AXES.iter().enumerate() {
                let dv = drag_value(commands, &fonts.ui, axis, *color, init[i], speed.max(0.001));
                let set = write(section.0, type_path, path);
                bind_2way(
                    commands,
                    dv,
                    move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                        Some(FieldValue::Vec3(a)) => a[i],
                        _ => 0.0,
                    },
                    move |w: &mut World, v: &f32| {
                        // Read-modify-write: the widget owns one axis, the field
                        // is all three.
                        let mut a =
                            match reflect_source::read_field(w, section.0, type_path, path, false) {
                                Some(FieldValue::Vec3(a)) => a,
                                _ => [0.0; 3],
                            };
                        a[i] = *v;
                        set(w, FieldValue::Vec3(a));
                    },
                );
                fill_control(commands, dv);
                commands.entity(wrap).add_child(dv);
            }
            wrap
        }
        FieldType::Bool => {
            let init = matches!(field.value, FieldValue::Bool(true));
            let sw = toggle_switch(commands, init);
            let set = write(section.0, type_path, path);
            bind_2way(
                commands,
                sw,
                move |rx: &Rx| {
                    matches!(
                        tracked_read(rx, section, type_path, path, false),
                        Some(FieldValue::Bool(true))
                    )
                },
                move |w: &mut World, v: &bool| set(w, FieldValue::Bool(*v)),
            );
            sw
        }
        // The colour fields want `Clone` setters (they drive the picker and the
        // alpha slider from the same closure), so these write through
        // `write_field` directly rather than capturing the [`write`] helper,
        // which is not `Clone`.
        FieldType::Color => renzora_ember::inspector::color_field(
            commands,
            move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                Some(FieldValue::Color(c)) => c,
                _ => [0.0; 3],
            },
            move |w: &mut World, c: [f32; 3]| {
                reflect_source::write_field(w, section.0, type_path, path, FieldValue::Color(c));
            },
        ),
        FieldType::ColorRgba => renzora_ember::inspector::color_field_rgba(
            commands,
            move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                Some(FieldValue::ColorRgba(c)) => c,
                _ => [0.0; 4],
            },
            move |w: &mut World, c: [f32; 4]| {
                reflect_source::write_field(w, section.0, type_path, path, FieldValue::ColorRgba(c));
            },
        ),
        FieldType::String => {
            let init = match &field.value {
                FieldValue::String(s) => s.clone(),
                _ => String::new(),
            };
            let ti = text_input(commands, &fonts.ui, "—", &init);
            let set = write(section.0, type_path, path);
            bind_text_input(
                commands,
                ti,
                move |rx: &Rx| match tracked_read(rx, section, type_path, path, false) {
                    Some(FieldValue::String(s)) => s,
                    _ => String::new(),
                },
                move |w: &mut World, v: String| set(w, FieldValue::String(v)),
            );
            fill_control(commands, ti);
            ti
        }
        FieldType::Enum { options } => {
            let current = match &field.value {
                FieldValue::Enum(s) => s.clone(),
                _ => String::new(),
            };
            let refs: Vec<&str> = options.to_vec();
            let selected = options.iter().position(|o| *o == current).unwrap_or(0);
            let dd = dropdown(commands, fonts, &refs, selected);
            let set = write(section.0, type_path, path);
            // The dropdown works in option indices, the field in variant names.
            bind_2way(
                commands,
                dd,
                move |rx: &Rx| {
                    let current = match tracked_read(rx, section, type_path, path, false) {
                        Some(FieldValue::Enum(s)) => s,
                        _ => String::new(),
                    };
                    options.iter().position(|o| *o == current).unwrap_or(0)
                },
                move |w: &mut World, i: &usize| {
                    if let Some(opt) = options.get(*i) {
                        set(w, FieldValue::Enum((*opt).to_string()));
                    }
                },
            );
            fill_control(commands, dd);
            dd
        }
        // Anything reflection could describe but not edit: shown, not dropped,
        // so a selected resource is an honest inventory of itself.
        // `Asset`/`AssetCreatable` land here too — reflection never generates
        // them, so a drop target would be dead UI.
        _ => {
            let text = commands
                .spawn((
                    Text::new(format_value(Some(&field.value))),
                    ui_font(&fonts.mono, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            bind_text(commands, text, move |rx: &Rx| {
                format_value(tracked_read(rx, section, type_path, path, true).as_ref())
            });
            text
        }
    }
}

/// A muted one-liner, for the several cases where there are no fields to show.
fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

// ── the list ─────────────────────────────────────────────────────────────────

/// One row's data, owned by the snapshot's build closure.
#[derive(Clone)]
struct RowView {
    type_path: &'static str,
    display: String,
    module: String,
    selected: bool,
}

fn build_list_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    view: &RowView,
    index: usize,
) -> Entity {
    let name = commands
        .spawn((
            Text::new(view.display.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(6.0),
            ..default()
        })
        .id();
    // The module path is what distinguishes the two `Assets` and the four
    // `Time`s a real world contains, and it is the first thing you need when a
    // name is unfamiliar.
    let module = commands
        .spawn((
            Text::new(view.module.clone()),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_shrink: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            // Striped by *full* index, not by position in the built window —
            // otherwise the stripes would crawl as you scroll.
            BackgroundColor(if view.selected {
                rgb(selection())
            } else {
                inspector_stripe(index)
            }),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            ResourceRow(view.type_path),
            Name::new("resource-row"),
        ))
        .id();
    commands.entity(row).add_children(&[name, spacer, module]);
    row
}

/// The virtualized list's snapshot: one item per indexed resource.
fn list_snapshot(rx: &Rx) -> KeyedSnapshot {
    let selected = rx.get_resource::<ResourceBrowser>().and_then(|b| b.selected);
    let views: Vec<RowView> = rx
        .get_resource::<ResourceIndex>()
        .map(|index| {
            index
                .rows
                .iter()
                .map(|row| RowView {
                    type_path: row.type_path,
                    display: row.display.clone(),
                    module: row.module.clone(),
                    selected: selected == Some(row.type_path),
                })
                .collect()
        })
        .unwrap_or_default();

    // Keyed on the resource, hashed on what the row draws — so selecting a row
    // rebuilds two rows, not the list.
    let items = views
        .iter()
        .map(|v| {
            (
                key_hash(v.type_path),
                hash_of(|h| {
                    v.type_path.hash(h);
                    v.selected.hash(h);
                }),
            )
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| build_list_row(commands, fonts, &views[i], i)),
    }
}

// ── the detail pane ──────────────────────────────────────────────────────────

/// The selected resource's fields.
///
/// Gated by a token rather than left to run every frame: this walks the whole
/// resource through reflection, which is exactly the work the master/detail
/// shape exists to do once per selection.
fn detail_snapshot(rx: &Rx) -> KeyedSnapshot {
    let world = rx.untracked();
    let Some(selected) = world
        .get_resource::<ResourceBrowser>()
        .and_then(|b| b.selected)
    else {
        return single_note("Select a resource to inspect it.");
    };
    let Some((entity, cid, type_path)) = world
        .get_resource::<ResourceIndex>()
        .and_then(|index| index.rows.iter().find(|r| r.type_path == selected))
        .map(|r| (r.entity, r.cid, r.type_path))
    else {
        return single_note("That resource is no longer in the world.");
    };

    let fields = reflect_source::resource_fields(world, entity, type_path);
    if fields.is_empty() {
        return single_note("No inspectable fields.");
    }

    // Keyed on the field path: stable for the life of a selection, and distinct
    // between selections because the token rebuilds the whole body anyway.
    let items = fields
        .iter()
        .map(|f| {
            let key = hash_of(|h| {
                type_path.hash(h);
                f.path.hash(h);
            });
            (key, key)
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let field = &fields[i];
            let control = build_control(commands, fonts, (entity, cid), type_path, field);
            let row = inspector_row(commands, &fonts.ui, field.label, control);
            commands
                .entity(row)
                .insert(BackgroundColor(inspector_stripe(i)));
            row
        }),
    }
}

fn single_note(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(key_hash(text), key_hash(text))],
        build: Box::new(move |commands, fonts, _| note(commands, fonts, text)),
    }
}

/// Everything the detail body depends on: which resource is selected, and
/// whether the index still holds it.
fn detail_token(rx: &Rx) -> u64 {
    let selected = rx.get_resource::<ResourceBrowser>().and_then(|b| b.selected);
    let generation = rx
        .get_resource::<ResourceIndex>()
        .map(|i| i.generation)
        .unwrap_or_default();
    hash_of(|h| {
        selected.hash(h);
        generation.hash(h);
    })
}

// ── panel content ────────────────────────────────────────────────────────────

fn build_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("resources-root"),
        ))
        .id();

    // ── toolbar ──
    let search = text_input(commands, &fonts.ui, "Search resources", "");
    commands.entity(search).insert((
        ResourceSearchInput,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(56.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let count = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    bind_text(commands, count, |rx: &Rx| {
        let Some(index) = rx.get_resource::<ResourceIndex>() else {
            return String::new();
        };
        let mut out = if index.rows.len() == index.total {
            format!("{}", index.total)
        } else {
            format!("{} / {}", index.rows.len(), index.total)
        };
        // Say what the list cannot show rather than letting the number read as
        // "this is every resource".
        if index.unreflected > 0 {
            out.push_str(&format!("  (+{} unreflected)", index.unreflected));
        }
        out
    });
    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("resources-toolbar"),
        ))
        .id();
    commands.entity(toolbar).add_children(&[search, count]);

    // ── master: the virtualized list ──
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("resources-list"),
        ))
        .id();
    virtual_scroll_versioned(
        commands,
        list,
        OVERSCAN,
        // The list's shape follows the index's generation; the selection is in
        // here too because it changes what two of the rows draw.
        |rx: &Rx| {
            let generation = rx
                .get_resource::<ResourceIndex>()
                .map(|i| i.generation)
                .unwrap_or_default();
            let selected = rx.get_resource::<ResourceBrowser>().and_then(|b| b.selected);
            hash_of(|h| {
                generation.hash(h);
                selected.hash(h);
            })
        },
        list_snapshot,
    );
    // `scroll_view` already returns a `flex_grow: 1, flex_basis: 0` wrapper, so
    // the list and the detail pane below split the panel's height between them.
    // Don't insert a `Node` over it — that would drop the relative positioning
    // the scrollbar track is anchored to.
    let list_scroll = scroll_view(commands, list);

    let divider = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id();

    // ── detail: the selected resource ──
    let title = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    bind_text(commands, title, |rx: &Rx| {
        let Some(selected) = rx.get_resource::<ResourceBrowser>().and_then(|b| b.selected) else {
            return "No selection".to_string();
        };
        rx.get_resource::<ResourceIndex>()
            .and_then(|index| index.rows.iter().find(|r| r.type_path == selected))
            .map(|r| r.display.clone())
            .unwrap_or_else(|| short_type_name(selected))
    });
    let subtitle = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 9.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_shrink: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, subtitle, |rx: &Rx| {
        rx.get_resource::<ResourceBrowser>()
            .and_then(|b| b.selected)
            .map(module_path)
            .unwrap_or_default()
    });
    let detail_spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(6.0),
            ..default()
        })
        .id();
    let detail_header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("resources-detail-header"),
        ))
        .id();
    commands
        .entity(detail_header)
        .add_children(&[title, detail_spacer, subtitle]);

    let detail_body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            Name::new("resources-detail-body"),
        ))
        .id();
    // A keyed list rather than a second virtualized one: a single resource's
    // field count is bounded by its own struct, so windowing would add spacers
    // and measurement for a list that is a dozen rows long. The token is what
    // matters here — it keeps the reflection walk to once per selection.
    keyed_list_tokened(commands, detail_body, detail_token, detail_snapshot);
    let detail_scroll = scroll_view(commands, detail_body);

    commands
        .entity(root)
        .add_children(&[toolbar, list_scroll, divider, detail_header, detail_scroll]);
    root
}

// ── systems ──────────────────────────────────────────────────────────────────

/// Mirror the search box into [`ResourceBrowser`].
fn sync_search(
    inputs: Query<&EmberTextInput, With<ResourceSearchInput>>,
    mut browser: ResMut<ResourceBrowser>,
) {
    for input in &inputs {
        if browser.filter != input.value {
            browser.filter = input.value.clone();
            browser.refresh = true;
        }
    }
}

/// Select the clicked row.
fn select_row(
    clicked: Query<(&Interaction, &ResourceRow), Changed<Interaction>>,
    mut browser: ResMut<ResourceBrowser>,
) {
    for (interaction, row) in &clicked {
        if *interaction == Interaction::Pressed && browser.selected != Some(row.0) {
            browser.selected = Some(row.0);
        }
    }
}

/// How often to re-scan the world's resource table when nothing the user did
/// could have changed it. Membership changes come from a plugin loading or a
/// system inserting a resource — nobody is waiting on those, and this scan is
/// the one part of the panel that costs something per frame.
const SCAN_EVERY: u32 = 12;

/// Rebuild [`ResourceIndex`] when the world's resources (or the filter) change.
fn refresh_index(world: &mut World, mut ticks: Local<u32>) {
    *ticks = ticks.wrapping_add(1);
    let refresh = world.resource::<ResourceBrowser>().refresh;
    if !refresh && !(*ticks).is_multiple_of(SCAN_EVERY) {
        return;
    }
    if refresh {
        // Only written when it was actually set: `ResourceBrowser` is a tracked
        // dependency of the list and the detail pane, and touching it every scan
        // would invalidate both twelve times a second for nothing.
        world.resource_mut::<ResourceBrowser>().refresh = false;
    }

    let filter = world
        .resource::<ResourceBrowser>()
        .filter
        .to_ascii_lowercase();
    let listed = reflect_source::world_resources(world);
    let total = listed.reflected.len();
    let unreflected = listed.unreflected;
    let mut rows: Vec<IndexRow> = listed
        .reflected
        .into_iter()
        .map(|entry| IndexRow {
            type_path: entry.type_path,
            display: short_type_name(entry.type_path),
            module: module_path(entry.type_path),
            entity: entry.entity,
            cid: entry.cid,
        })
        .filter(|row| {
            filter.is_empty()
                || row.display.to_ascii_lowercase().contains(&filter)
                || row.type_path.to_ascii_lowercase().contains(&filter)
        })
        .collect();
    rows.sort_by(|a, b| {
        a.display
            .cmp(&b.display)
            .then_with(|| a.type_path.cmp(b.type_path))
    });

    // Membership and order only. Field *values* are deliberately absent: they
    // are reactive, and folding them in here would invalidate the whole list
    // every frame any resource changed.
    let generation = hash_of(|h| {
        for row in &rows {
            row.type_path.hash(h);
            // The resource's hidden entity moves if it is removed and
            // re-inserted, and every row's bindings address it by entity — so a
            // stale index would keep reading a despawned one.
            row.entity.hash(h);
        }
    });
    let index = world.resource::<ResourceIndex>();
    if index.generation == generation && index.total == total && index.unreflected == unreflected {
        return;
    }
    let mut index = world.resource_mut::<ResourceIndex>();
    index.generation = generation;
    index.total = total;
    index.unreflected = unreflected;
    index.rows = rows;
}

pub fn register(app: &mut App) {
    app.init_resource::<ResourceBrowser>();
    app.init_resource::<ResourceIndex>();
    // Panel metadata (title, icon, category) is seeded by the shell's
    // `PANEL_META` table alongside every other built-in panel.
    app.register_panel_content(PANEL_ID, false, build_content)
        // `systems` already gates these on the panel being the visible tab —
        // there is nothing to keep in sync while nobody is looking at it.
        .systems(Update, (sync_search, select_row, refresh_index).chain());
}
