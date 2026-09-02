//! The asset field: a drop target for a drag from the asset browser, a clear
//! button, and — for creatable fields — a "+" that authors the file in place.
//!
//! [`asset_drop_field`] is public because component drawers in other crates want
//! the same slot; the drop / clear / highlight systems registered by
//! [`super::register`] drive any `AssetDropZone`, wherever it was built.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::{EditorCommands, FieldValue};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_with;
use renzora_theme::ThemeManager;

use super::spec::c32;
use super::{c, record_field_change, GetFn, Mutate, SetFn};

#[derive(Component)]
pub(super) struct AssetDropZone {
    extensions: Vec<String>,
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
    field_name: &'static str,
}

#[derive(Component)]
pub(super) struct AssetClearBtn {
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
    field_name: &'static str,
}

/// `(display text, has-value)` for an asset field value (filename or prompt).
fn asset_display(v: Option<FieldValue>) -> (String, bool) {
    match v {
        Some(FieldValue::Asset(Some(p))) if !p.is_empty() => {
            let name = std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(p);
            (name, true)
        }
        _ => (renzora::lang::t("inspector.drag_asset"), false),
    }
}

/// Reusable asset-drop field (drag from the asset browser + clear button), for
/// component drawers outside this crate. The drop / clear / highlight systems
/// registered by [`super::register`] drive any `AssetDropZone`, so callers only
/// supply get/set fn-pointers (using `FieldValue::Asset`) and the accepted
/// extensions. Returns the row entity to place inside an `inspector_row`.
pub fn asset_drop_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: fn(&World, Entity) -> Option<FieldValue>,
    set_fn: fn(&mut World, Entity, FieldValue),
    extensions: Vec<String>,
) -> Entity {
    // Public signature deliberately still takes fn pointers — external drawers
    // have nothing to capture, and widening it would churn every caller.
    build_asset_field(
        commands,
        fonts,
        entity,
        "asset",
        std::sync::Arc::new(get_fn),
        std::sync::Arc::new(set_fn),
        extensions,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_asset_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    field_name: &'static str,
    get_fn: GetFn,
    set_fn: SetFn,
    extensions: Vec<String>,
    create_fn: Option<Mutate>,
) -> Entity {
    // One handle per `move` closure below: each takes ownership of what it
    // captures, so they cannot share a single binding.
    let get_r = get_fn.clone();
    let get_r2 = get_fn.clone();
    let path_text = commands
        .spawn((
            Text::new(renzora::lang::t("inspector.drag_asset")),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(renzora_ember::theme::text_muted())),
            bevy::text::TextLayout::no_wrap(),
            FocusPolicy::Pass,
        ))
        .id();
    bind_with(
        commands,
        path_text,
        move |w| asset_display(get_r(w.untracked(), entity)),
        |w, e, (text, has): &(String, bool)| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                if t.0 != *text {
                    t.0 = text.clone();
                }
            }
            if let Some(mut col) = w.get_mut::<TextColor>(e) {
                col.0 = c(if *has { (210, 210, 220) } else { (140, 140, 152) });
            }
        },
    );
    let drop_box = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(c((28, 28, 34))),
            BorderColor::all(c((70, 70, 82))),
            bevy::ui::RelativeCursorPosition::default(),
            AssetDropZone {
                extensions,
                get_fn: get_fn.clone(),
                set_fn: set_fn.clone(),
                entity,
                field_name,
            },
            Name::new("asset-drop"),
        ))
        .id();
    commands.entity(drop_box).add_child(path_text);

    // The clear button, as a Phosphor `x` rather than a literal `\u{2715}`
    // (MULTIPLICATION X) set in the UI font. The UI font has no glyph at that
    // codepoint, so it rendered as a tofu box — a blank square sitting where a
    // "remove this template" control should be, next to the revert arrow. The
    // neighbouring "+" survived only because it is ASCII.
    let clear = renzora_ember::font::icon_text(
        commands,
        &fonts.phosphor,
        "x",
        renzora_ember::theme::text_muted(),
        11.0,
    );
    commands.entity(clear).insert((
        Node {
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(2.0)),
            ..default()
        },
        Interaction::default(),
        AssetClearBtn {
            get_fn: get_fn.clone(),
            set_fn: set_fn.clone(),
            entity,
            field_name,
        },
        Name::new("asset-clear"),
    ));

    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("asset-field"),
        ))
        .id();
    commands.entity(row).add_children(&[drop_box, clear]);

    // Creatable fields get a "+" that authors a fresh asset in place (e.g. a
    // world-UI panel's template), so an empty field isn't a dead end — you don't
    // have to go make the file in the browser first and drag it back.
    if let Some(create_fn) = create_fn {
        let plus = commands
            .spawn((
                Text::new("\u{FF0B}"), // ＋ (full-width, reads as a button glyph)
                ui_font(&fonts.ui, 11.0),
                TextColor(c(renzora_ember::theme::text_muted())),
                Node {
                    padding: UiRect::horizontal(Val::Px(2.0)),
                    ..default()
                },
                Interaction::default(),
                AssetCreateBtn {
                    create_fn: create_fn.clone(),
                    get_fn: get_fn.clone(),
                    entity,
                },
                Name::new("asset-create"),
            ))
            .id();
        // Only offer "+" while the field is empty — once it points at an asset,
        // the ✕ clears it and drag-drop replaces it. Reactive so it hides the
        // instant a template is created/assigned.
        bind_with(
            commands,
            plus,
            move |w| matches!(get_r2(w.untracked(), entity), Some(FieldValue::Asset(Some(_)))),
            |w, e, has: &bool| {
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    let want = if *has { Display::None } else { Display::Flex };
                    if node.display != want {
                        node.display = want;
                    }
                }
            },
        );
        commands.entity(row).add_child(plus);
    }
    row
}

/// The "+" on an [`FieldType::AssetCreatable`](renzora::FieldType) field. Runs
/// `create_fn` to author + assign the asset in place, but only when the field is
/// still empty — once it points at something, the ✕ clears and drag-drop replaces.
#[derive(Component)]
pub(super) struct AssetCreateBtn {
    create_fn: Mutate,
    get_fn: GetFn,
    entity: Entity,
}

pub(super) fn asset_create_click(
    q: Query<(&Interaction, &AssetCreateBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (create_fn, get_fn, entity) = (btn.create_fn.clone(), btn.get_fn.clone(), btn.entity);
        cmds.push(move |w: &mut World| {
            // Guard against a double-fire creating two files: skip if the field
            // already has a value (e.g. a race with drag-drop).
            if !matches!(get_fn(w, entity), Some(FieldValue::Asset(Some(_)))) {
                create_fn(w, entity);
            }
        });
    }
}

/// Drop an asset (dragged from the asset browser) onto the hovered, extension-
/// matching field → set its project-relative path.
pub(super) fn asset_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&bevy::ui::RelativeCursorPosition, &AssetDropZone)>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else {
        return;
    };
    if !payload.is_detached {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let ext_refs: Vec<&str> = zone.extensions.iter().map(|s| s.as_str()).collect();
        if !ext_refs.is_empty() && !payload.matches_extensions(&ext_refs) {
            continue;
        }
        let path_str = project
            .as_ref()
            .map(|p| p.make_asset_relative(&payload.path))
            .unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        let (get_fn, set_fn, entity, name) = (zone.get_fn.clone(), zone.set_fn.clone(), zone.entity, zone.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Asset(Some(path_str.clone())))
        });
        break;
    }
}

pub(super) fn asset_clear_click(
    q: Query<(&Interaction, &AssetClearBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (get_fn, set_fn, entity, name) = (btn.get_fn.clone(), btn.set_fn.clone(), btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Asset(None))
        });
    }
}

/// Highlight a drop zone's border while a compatible asset is dragged over it.
pub(super) fn asset_drop_highlight(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    theme: Option<Res<ThemeManager>>,
    mut zones: Query<(&bevy::ui::RelativeCursorPosition, &AssetDropZone, &mut BorderColor)>,
) {
    let accent = theme
        .map(|t| c(c32(t.active_theme.semantic.accent)))
        .unwrap_or(c((120, 140, 200)));
    for (rcp, zone, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| {
            let ext_refs: Vec<&str> = zone.extensions.iter().map(|s| s.as_str()).collect();
            p.is_detached
                && rcp.cursor_over
                && (ext_refs.is_empty() || p.matches_extensions(&ext_refs))
        });
        let want = BorderColor::all(if active { accent } else { c((70, 70, 82)) });
        if *bc != want {
            *bc = want;
        }
    }
}
