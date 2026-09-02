//! The material slot: preview square, picker field, and the buttons inside it.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::MaterialThumbnailRegistry;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_with;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{
    accent, border, faint_bg, hover_bg, placeholder, popup_bg, rgb, section_bg, text_muted,
    text_primary,
};
use renzora_ember::widgets::{HoverTint, HoverTooltip};

use super::{
    material_abs, material_path, MatClearBtn, MatCreateBtn, MatDropZone, MatEditBtn,
};

/// Side of the slot's preview square. The field is given the same height, so the
/// two line up as one band with the action row tucked underneath.
const SLOT_PREVIEW: f32 = 40.0;

/// A material preview square: the rendered thumbnail when one exists, a muted
/// sphere glyph when it doesn't. Returns `(square, fallback_glyph)`; feed both
/// to [`bind_preview`].
///
/// The fallback earns its keep. A `.material` thumbnail is a separate one-shot
/// render that may not have landed yet — and never lands for a material that
/// fails to compile — and an `ImageNode` holding a default handle draws
/// *nothing*, so the old slot was a flat dark hole for most of the time it was
/// on screen. A framed square with a glyph in it reads as "preview pending"
/// rather than "broken", which matters far more now that the picker is a grid
/// of these.
pub(super) fn preview_square(
    commands: &mut Commands,
    fonts: &EmberFonts,
    size: f32,
    radius: f32,
    glyph_size: f32,
) -> (Entity, Entity) {
    let square = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(radius)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border()).with_alpha(0.55)),
            ImageNode::new(Handle::default()),
            bevy::ui::FocusPolicy::Pass,
            Name::new("material-preview"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, "sphere", placeholder(), glyph_size);
    commands.entity(glyph).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(square).add_child(glyph);
    (square, glyph)
}

/// Point a [`preview_square`] at whatever thumbnail `thumb` resolves to, hiding
/// the fallback glyph exactly while an image is bound (otherwise the glyph would
/// keep drawing on top of the render once it arrives).
pub(super) fn bind_preview<F>(commands: &mut Commands, square: Entity, glyph: Entity, thumb: F)
where
    F: for<'w> Fn(&Rx<'w>) -> Option<Handle<Image>> + Send + Sync + 'static,
{
    bind_with(commands, square, thumb, move |w, e, h: &Option<Handle<Image>>| {
        if let Some(mut img) = w.get_mut::<ImageNode>(e) {
            img.image = h.clone().unwrap_or_default();
        }
        if let Some(mut n) = w.get_mut::<Node>(glyph) {
            let want = if h.is_some() { Display::None } else { Display::Flex };
            if n.display != want {
                n.display = want;
            }
        }
    });
}

pub(super) fn build_slot(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, path: &str) -> Entity {
    let has_mat = !path.is_empty();
    let name = if has_mat {
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        "No material".to_string()
    };
    // Second line of the field. For a bound material it's where the file lives;
    // for an empty slot it's the instruction, because telling you what to do
    // with it is the only job an empty slot has.
    let sub = if has_mat {
        std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "project root".to_string())
    } else {
        "Click to pick, or drop a .material".to_string()
    };

    // The slot is a column: the header (preview + field + actions) with the
    // picker tray under it. The tray is **in flow**, so opening it pushes the
    // texture slots down rather than floating over them — the drawer stays one
    // readable column instead of growing a second layer.
    let slot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    // The header, and the drop zone (material + image extensions). No card fill
    // behind it: the field and the action chips carry their own surfaces, and a
    // filled box around them was a third nested rectangle saying nothing. The
    // transparent border stays because `mat_slot_drop_highlight` accents it
    // while a compatible file is dragged over.
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            RelativeCursorPosition::default(),
            MatDropZone { entity },
            Name::new("material-slot"),
        ))
        .id();

    let (thumb, thumb_glyph) = preview_square(commands, fonts, SLOT_PREVIEW, 5.0, 17.0);
    bind_preview(commands, thumb, thumb_glyph, move |w| {
        let path = material_path(&Rx::new(w.untracked()), entity);
        material_abs(&Rx::new(w.untracked()), &path)
            .and_then(|abs| w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs)))
    });

    // Right column: the picker field over the action row.
    let col = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();

    let panel = super::picker::build_picker_panel(commands, fonts, entity);

    // The field: name over folder, caret on the right, and the whole thing is
    // the picker trigger. It replaces a one-line button, a loose folder caption
    // and a "browse" icon that opened this same list — three pieces of chrome
    // all answering "which material is this".
    let field = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(SLOT_PREVIEW),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            HoverTint::solid(rgb(popup_bg()), rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            // No tooltip: a field with a caret on it already reads as a picker,
            // and this one is big enough to hover by accident on the way to the
            // action chips under it.
            Name::new("material-name"),
        ))
        .id();
    // A clip wrapper, because `Overflow::clip` clips a node's *children*: on the
    // text nodes themselves a long material name would spill over the caret.
    let text_col = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let name_text = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(if has_mat { text_primary() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let sub_text = commands
        .spawn((
            Text::new(sub),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(placeholder())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(text_col).add_children(&[name_text, sub_text]);
    // The caret is repointed (not respawned) by `mat_picker_toggle`, so it also
    // reports the tray's state instead of permanently promising "down".
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(field).insert(super::picker::MatPickerToggle { entity, panel, caret });

    // Edit and remove live *inside* the field, just left of the caret, rather
    // than on a row of their own underneath it. They act on the material the
    // field names, so that's where they belong — and the row they used to sit on
    // was a second line of chrome for two glyphs.
    //
    // Nesting them in the picker's own trigger is safe because `chip_btn` blocks
    // focus: a press on a chip doesn't fall through to the field behind it, so
    // clicking ✕ doesn't also slide the tray open. Same mechanism the
    // texture-slot clear already relies on inside its row.
    let mut field_kids = vec![text_col];
    if has_mat {
        let edit = icon_btn(commands, fonts, "pencil-simple", "Open in the material editor");
        commands.entity(edit).insert(MatEditBtn { entity });
        let clear = icon_btn(commands, fonts, "x", "Remove this material");
        commands.entity(clear).insert(MatClearBtn { entity });
        field_kids.extend_from_slice(&[edit, clear]);
    }
    field_kids.push(caret);
    commands.entity(field).add_children(&field_kids);

    let mut col_kids = vec![field];
    if !has_mat {
        // An empty slot has exactly one sensible move, so it gets exactly one
        // button — and this one keeps its label, because there's no material to
        // reason from and a lone "+" would be a guess.
        let actions = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, ..default() })
            .id();
        let create = chip_btn(commands, fonts, "plus", Some("New material"), None, rgb(section_bg()), text_primary());
        commands.entity(create).insert(MatCreateBtn { entity });
        commands.entity(actions).add_child(create);
        col_kids.push(actions);
    }

    commands.entity(col).add_children(&col_kids);
    commands.entity(row).add_children(&[thumb, col]);
    commands.entity(slot).add_children(&[row, panel]);
    slot
}

/// A bare glyph button — the field's edit/remove, the texture-slot clear and the
/// override revert. Each sits *inside* something that already has a surface, so
/// it stays transparent until hovered.
pub(super) fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tooltip: &str) -> Entity {
    chip_btn(commands, fonts, icon, None, Some(tooltip), Color::NONE, text_muted())
}

/// A small button: a glyph, optionally a label, optionally a tooltip.
///
/// `tooltip` is optional because a *labelled* button doesn't want one — the
/// label already says it, and a bubble repeating the word under the cursor is
/// noise.
fn chip_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: Option<&str>,
    tooltip: Option<&str>,
    base: Color,
    fg: (u8, u8, u8),
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(22.0),
                min_width: Val::Px(24.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(if label.is_some() { 8.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(base),
            HoverTint::solid(base, rgb(hover_bg()), rgb(accent()).with_alpha(0.35)),
            Interaction::default(),
            // Block, or the press also lands on whatever sits under the button —
            // for the per-slot clear that is the slot row itself, which would
            // open a file dialog on the same click that emptied the slot, and
            // for the field's edit/remove it is the picker trigger.
            bevy::ui::FocusPolicy::Block,
            Name::new("material-icon-btn"),
        ))
        .id();
    if let Some(tooltip) = tooltip {
        commands.entity(btn).insert(HoverTooltip::new(tooltip));
    }
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 12.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
    if let Some(label) = label {
        let text = commands
            .spawn((
                Text::new(label),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(fg)),
                bevy::text::TextLayout::no_wrap(),
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        commands.entity(btn).add_child(text);
    }
    btn
}
