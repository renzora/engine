//! Syncs individual style components → bevy_ui components (`BackgroundColor`,
//! `BorderColor`, `Node.border`, `Node.border_radius`, `Node.padding`, `Node.overflow`).
//!
//! Runs every frame on changed style components so that the editor
//! inspector changes are immediately reflected in the runtime bevy_ui rendering.

use bevy::prelude::*;

use crate::components::*;

/// Applies style component fields to the corresponding bevy_ui components.
pub fn apply_widget_style_system(
    mut query: Query<
        (
            Option<&UiFill>,
            Option<&UiStroke>,
            Option<&UiBorderRadius>,
            Option<&UiOpacity>,
            Option<&UiClipContent>,
            Option<&UiTextStyle>,
            Option<&UiPadding>,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&mut TextColor>,
        ),
        (
            With<UiWidget>,
            Or<(
                Changed<UiFill>,
                Changed<UiStroke>,
                Changed<UiBorderRadius>,
                Changed<UiOpacity>,
                Changed<UiClipContent>,
                Changed<UiTextStyle>,
                Changed<UiPadding>,
            )>,
        ),
    >,
) {
    for (fill, stroke, border_radius, opacity, clip_content, text, padding, mut node, bg, border, text_color) in &mut query {
        let alpha = opacity.map(|o| o.0).unwrap_or(1.0);

        // ── Fill → BackgroundColor ───────────────────────────────────────
        if let Some(fill) = fill {
            let fill_color = fill.primary_color();
            if let Some(mut bg) = bg {
                let mut c = fill_color.to_srgba();
                c.alpha *= alpha;
                bg.0 = c.into();
            }
        }

        // ── Stroke → BorderColor + Node.border ─────────────────────────
        if let Some(stroke) = stroke {
            if !stroke.is_none() {
                let w = stroke.width;
                let sides = &stroke.sides;
                node.border = UiRect {
                    top: if sides.top { Val::Px(w) } else { Val::Px(0.0) },
                    right: if sides.right { Val::Px(w) } else { Val::Px(0.0) },
                    bottom: if sides.bottom { Val::Px(w) } else { Val::Px(0.0) },
                    left: if sides.left { Val::Px(w) } else { Val::Px(0.0) },
                };
                if let Some(mut bc) = border {
                    *bc = BorderColor::all(stroke.color);
                }
            } else {
                node.border = UiRect::ZERO;
            }
        }

        // ── Border Radius ───────────────────────────────────────────────
        if let Some(br) = border_radius {
            node.border_radius = br.to_bevy();
        }

        // ── Padding ─────────────────────────────────────────────────────
        if let Some(padding) = padding {
            node.padding = UiRect {
                top: Val::Px(padding.top),
                right: Val::Px(padding.right),
                bottom: Val::Px(padding.bottom),
                left: Val::Px(padding.left),
            };
        }

        // ── Clip ────────────────────────────────────────────────────────
        if let Some(clip) = clip_content {
            node.overflow = if clip.0 {
                Overflow::clip()
            } else {
                Overflow::visible()
            };
        }

        // ── Text style → TextColor ──────────────────────────────────────
        if let Some(text) = text {
            if let Some(mut tc) = text_color {
                tc.0 = text.color;
            }
        }
    }
}

/// Ensures every styled entity has the required bevy_ui components
/// so the sync system can write to them. Runs once on insertion.
/// Shape widgets (with `UiShapeWidget`) are excluded — they use `MaterialNode` instead.
pub fn ensure_style_components(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            Option<&UiFill>,
            Option<&UiStroke>,
            Option<&BackgroundColor>,
            Option<&BorderColor>,
        ),
        (Added<UiWidget>, Without<crate::shapes::UiShapeWidget>),
    >,
) {
    for (entity, fill, stroke, bg, border) in &query {
        let mut ec = commands.entity(entity);
        if bg.is_none() {
            if let Some(fill) = fill {
                ec.insert(BackgroundColor(fill.primary_color()));
            }
        }
        if border.is_none() {
            if let Some(stroke) = stroke {
                if !stroke.is_none() {
                    ec.insert(BorderColor::all(stroke.color));
                }
            }
        }
    }
}
