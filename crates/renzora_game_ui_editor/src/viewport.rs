//! The canvas viewport: a dark area holding the zoomed "design frame" whose
//! `ImageNode` shows the live offscreen render of the game UI
//! (`renzora_game_ui::canvas_render::UiCanvasRender`). The frame is sized to the
//! active canvas's reference resolution × zoom, so it shows the UI at design
//! scale.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_display, bind_with};
use renzora_ember::theme::*;
use renzora_game_ui::canvas_render::UiCanvasRender;

use crate::NativeCanvasState;

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            Name::new("ui-canvas-viewport"),
        ))
        .id();

    // "No canvas" note.
    let note = commands
        .spawn((Text::new("No UI canvas in the scene"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
        .id();
    bind_display(commands, note, |w| w.get_resource::<NativeCanvasState>().is_none_or(|s| s.active_canvas.is_none()));

    // The design frame — sized to reference resolution × zoom.
    let frame = commands
        .spawn((
            Node { width: Val::Px(1280.0), height: Val::Px(720.0), flex_shrink: 0.0, border: UiRect::all(Val::Px(1.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.03)),
            BorderColor::all(rgb(border())),
            Name::new("ui-canvas-frame"),
        ))
        .id();
    bind_display(commands, frame, |w| w.get_resource::<NativeCanvasState>().is_some_and(|s| s.active_canvas.is_some()));
    bind_with(
        commands,
        frame,
        |w| {
            let s = w.get_resource::<NativeCanvasState>();
            let zoom = s.map(|s| s.zoom).unwrap_or(1.0);
            let (cw, ch) = s.map(|s| (s.canvas_width, s.canvas_height)).unwrap_or((1280.0, 720.0));
            (cw * zoom, ch * zoom)
        },
        |w, e, (fw, fh): &(f32, f32)| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                let (pw, ph) = (Val::Px(*fw), Val::Px(*fh));
                if n.width != pw {
                    n.width = pw;
                }
                if n.height != ph {
                    n.height = ph;
                }
            }
        },
    );

    // The rendered UI image, filling the frame.
    let img = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            Name::new("ui-canvas-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<UiCanvasRender>().map(|r| r.image_handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );
    // Editing overlay (selection box + handles + hit layer) over the image.
    let overlay = crate::overlay::build(commands);
    commands.entity(frame).add_children(&[img, overlay]);

    commands.entity(area).add_children(&[note, frame]);
    area
}
