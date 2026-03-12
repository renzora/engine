//! Syncs `UiWidgetStyle` → bevy_ui components (`BackgroundColor`, `BorderColor`,
//! `Node.border`, `Node.border_radius`, `Node.padding`, `Node.overflow`).
//!
//! Runs every frame on changed `UiWidgetStyle` components so that the editor
//! inspector changes are immediately reflected in the runtime bevy_ui rendering.

use bevy::prelude::*;

use crate::components::UiWidgetStyle;

/// Applies `UiWidgetStyle` fields to the corresponding bevy_ui components.
pub fn apply_widget_style_system(
    mut query: Query<
        (
            &UiWidgetStyle,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&mut TextColor>,
        ),
        Changed<UiWidgetStyle>,
    >,
) {
    for (style, mut node, bg, border, text_color) in &mut query {
        // ── Fill → BackgroundColor ───────────────────────────────────────
        let fill_color = style.fill.primary_color();
        if let Some(mut bg) = bg {
            let mut c = fill_color.to_srgba();
            c.alpha *= style.opacity;
            bg.0 = c.into();
        }

        // ── Stroke → BorderColor + Node.border ─────────────────────────
        if !style.stroke.is_none() {
            let w = style.stroke.width;
            let sides = &style.stroke.sides;
            node.border = UiRect {
                top: if sides.top { Val::Px(w) } else { Val::Px(0.0) },
                right: if sides.right { Val::Px(w) } else { Val::Px(0.0) },
                bottom: if sides.bottom { Val::Px(w) } else { Val::Px(0.0) },
                left: if sides.left { Val::Px(w) } else { Val::Px(0.0) },
            };
            if let Some(mut bc) = border {
                *bc = BorderColor::all(style.stroke.color);
            }
        } else {
            node.border = UiRect::ZERO;
        }

        // ── Border Radius ───────────────────────────────────────────────
        node.border_radius = style.border_radius.to_bevy();

        // ── Padding ─────────────────────────────────────────────────────
        node.padding = UiRect {
            top: Val::Px(style.padding.top),
            right: Val::Px(style.padding.right),
            bottom: Val::Px(style.padding.bottom),
            left: Val::Px(style.padding.left),
        };

        // ── Clip ────────────────────────────────────────────────────────
        node.overflow = if style.clip_content {
            Overflow::clip()
        } else {
            Overflow::visible()
        };

        // ── Text style → TextColor ──────────────────────────────────────
        if let Some(mut tc) = text_color {
            tc.0 = style.text.color;
        }
    }
}

/// Ensures every `UiWidgetStyle` entity has the required bevy_ui components
/// so the sync system can write to them. Runs once on insertion.
pub fn ensure_style_components(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &UiWidgetStyle,
            Option<&BackgroundColor>,
            Option<&BorderColor>,
        ),
        Added<UiWidgetStyle>,
    >,
) {
    for (entity, style, bg, border) in &query {
        let mut ec = commands.entity(entity);
        if bg.is_none() {
            ec.insert(BackgroundColor(style.fill.primary_color()));
        }
        if border.is_none() && !style.stroke.is_none() {
            ec.insert(BorderColor::all(style.stroke.color));
        }
    }
}
