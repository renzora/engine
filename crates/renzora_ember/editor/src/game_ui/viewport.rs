//! The canvas viewport: a dark area holding the zoomed "design frame" whose
//! `ImageNode` shows the live offscreen render of the game UI
//! (`crate::game_ui::canvas_render::UiCanvasRender`). The frame is sized to the
//! active canvas's reference resolution × zoom, so it shows the UI at design
//! scale.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_with};
use renzora_ember::theme::*;
use crate::game_ui::canvas::UiCanvasPreviewEnabled;
use crate::game_ui::canvas_render::UiCanvasRender;
use crate::game_ui::NativeCanvasState;

/// The empty state's "Create one" button.
#[derive(Component)]
pub(crate) struct CreateCanvasBtn;

/// Spawn a canvas and select it, so the inspector opens on the UI Template slot
/// — which is the next thing to fill and the whole reason the entity exists.
pub(crate) fn create_canvas_click(
    q: Query<&Interaction, (With<CreateCanvasBtn>, Changed<Interaction>)>,
    cmds: Option<Res<renzora::EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        let canvas = super::register::spawn_ui_canvas(world);
        if let Some(sel) = world.get_resource::<renzora::EditorSelection>() {
            sel.set(Some(canvas));
        }
    });
}

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
            // Clicking the dark area around the frame clears the selection. The
            // centered frame sits on top where it covers, so a press only reaches
            // this node when it lands *outside* the frame.
            Interaction::default(),
            crate::game_ui::interaction::CanvasBackground,
            crate::game_ui::ruler::RulerArea,
            // The rulers' cursor markers measure against this node, so the
            // reading is valid over the whole canvas area — not just the frame.
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("ui-canvas-viewport"),
        ))
        .id();

    // The empty state: what is missing, and the one thing to do about it.
    //
    // It was a line of grey text and nothing else — a dead end in the panel you
    // had just gone to the trouble of opening. The panel knows exactly what is
    // wrong and exactly how to fix it, so it should offer to.
    let note = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("ui-canvas-empty"),
        ))
        .id();
    let note_text = commands
        .spawn((
            Text::new("No UI canvas in the scene"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let create = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            CreateCanvasBtn,
            Name::new("ui-canvas-create"),
        ))
        .id();
    let create_label = commands
        .spawn((
            Text::new("Create one"),
            ui_font(&fonts.ui, 11.5),
            TextColor(Color::WHITE),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(create).add_child(create_label);
    commands.entity(note).add_children(&[note_text, create]);
    bind_display(commands, note, |w| w.get_resource::<NativeCanvasState>().is_none_or(|s| s.active_canvas.is_none()));

    // The design frame — sized to reference resolution × zoom.
    let frame = commands
        .spawn((
            // Overflow visible so the selection handles (which extend a few px
            // beyond the widget rect) aren't clipped at the canvas edge.
            Node { width: Val::Px(1280.0), height: Val::Px(720.0), flex_shrink: 0.0, border: UiRect::all(Val::Px(1.0)), ..default() },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.03)),
            BorderColor::all(rgb(border())),
            bevy::ui::UiTransform::IDENTITY,
            crate::game_ui::nav::CanvasFrame,
            Name::new("ui-canvas-frame"),
        ))
        .id();
    bind_display(commands, frame, |w| w.get_resource::<NativeCanvasState>().is_some_and(|s| s.active_canvas.is_some()));
    // The frame's size is *not* bound here. It is written by `nav::apply_view`,
    // alongside the pan offset, so a zoom's move and resize land on the same
    // frame — see that function. As a `bind_with` it ran in `run_reactions`,
    // unordered against `apply_view`, and the split showed up as a twitch on
    // every scroll. The `Node` above carries the un-zoomed size as the starting
    // value; `apply_view` corrects it on the first tick.

    // Scene backdrop: the editor-camera render (the same slot-0 image the
    // viewport shows — 3D, or 2D when UI view was entered from the 2D view)
    // behind the UI, toggled by UiCanvasPreviewEnabled (default on).
    let backdrop = commands
        .spawn((
            // `Stretch`: the render target's aspect follows the viewport
            // panel's, which is not the canvas's reference aspect, and the
            // default mode letterboxed it inside the frame — a scene backdrop
            // with black bars down both sides reads as part of the UI.
            ImageNode { image_mode: bevy::ui::widget::NodeImageMode::Stretch, ..default() },
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            Name::new("ui-canvas-backdrop"),
        ))
        .id();
    // Two conditions, and the second is not a preference.
    //
    // An undocked viewport slot shrinks its render target to 64×64 — the always-
    // on atmosphere/IBL pass has to keep running, and shrinking it is what makes
    // that nearly free (see `UNDOCKED_TARGET_SIZE` in `renzora_viewport`). So in
    // a workspace with no viewport panel, the "scene behind your UI" is a 64px
    // square smeared across a 1280×720 frame: small, blurry, and the wrong
    // aspect. It is not a preview of anything.
    //
    // The backdrop is therefore only shown when a viewport is also on screen —
    // which is the arrangement where it earns its keep anyway, since you are
    // then placing a HUD over a scene you can see rendered properly next to it.
    bind_display(commands, backdrop, |w| {
        let wanted = w
            .get_resource::<UiCanvasPreviewEnabled>()
            .is_none_or(|r| r.0);
        wanted
            && renzora_ember::dock::panel_visible_anywhere(
                "viewport",
                w.get_resource::<renzora_ember::dock::Dock>(),
                w.get_resource::<renzora_ember::dock::FixedDock>(),
                w.get_resource::<renzora_ember::dock::DockWindows>(),
            )
    });
    bind_with(
        commands,
        backdrop,
        |w| w.get_resource::<renzora::ViewportRenderTarget>().and_then(|rt| rt.image.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );

    // The rendered UI image (transparent bg), filling the frame over the backdrop.
    let img = commands
        .spawn((
            ImageNode::default(),
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
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
    let overlay = crate::game_ui::overlay::build(commands, fonts);
    commands.entity(frame).add_children(&[backdrop, img, overlay]);

    // Rulers before the shelf, so the shelf floats over them rather than being
    // clipped by the strip it sits beside.
    let rulers = crate::game_ui::ruler::build(commands, fonts);
    let shelf = crate::game_ui::palette::build_shelf(commands, fonts);
    commands
        .entity(area)
        .add_children(&[note, frame, rulers, shelf]);
    area
}
