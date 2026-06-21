//! Native bevy_ui (ember) crash-report overlay — the editor-only replacement
//! for the old egui crash window. A dimmed backdrop + centered panel showing
//! the previous session's error / location / backtrace, with Copy-to-clipboard
//! and Close. Moved out of `renzora_engine::crash` so the lean runtime carries
//! no `renzora_ember` dependency; the data it reads (`CrashReport`,
//! `CrashReportWindowState`) still lives in `renzora_engine::crash`.

use renzora_engine::crash::{CrashReport, CrashReportWindowState};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{border, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{button, scroll_area, OverlaySurface};

#[derive(Component)]
pub(crate) struct CrashOverlayRoot;
#[derive(Component)]
pub(crate) struct CrashCloseButton;
#[derive(Component)]
pub(crate) struct CrashCopyButton;

/// Spawn the overlay when a previous crash is flagged; tear it down when cleared.
pub(crate) fn manage_crash_overlay(world: &mut World) {
    let show = world
        .get_resource::<CrashReportWindowState>()
        .is_some_and(|s| s.show_window && s.report.is_some());
    let mut q = world.query_filtered::<Entity, With<CrashOverlayRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if show && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            return;
        };
        let report = world
            .resource::<CrashReportWindowState>()
            .report
            .clone()
            .unwrap();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_overlay(&mut commands, &fonts, &report);
        }
        queue.apply(world);
    } else if !show && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn line(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    text: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color))))
        .id()
}

fn spawn_overlay(commands: &mut Commands, fonts: &EmberFonts, report: &CrashReport) {
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(9800),
            FocusPolicy::Block,
            Interaction::default(),
            OverlaySurface,
            CrashOverlayRoot,
            Name::new("crash-overlay"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(640.0),
                max_width: Val::Percent(94.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("crash-panel"),
        ))
        .id();

    let heading = line(
        commands,
        &fonts.ui,
        "The application crashed in the previous session",
        text_primary(),
        15.0,
    );
    let ts = line(
        commands,
        &fonts.ui,
        &format!("Timestamp: {}", report.timestamp),
        text_muted(),
        12.0,
    );
    let err_label = line(commands, &fonts.ui, "Error:", text_muted(), 12.0);
    let err = line(commands, &fonts.ui, &report.message, (235, 110, 110), 13.0);
    let loc_label = line(commands, &fonts.ui, "Location:", text_muted(), 12.0);
    let loc = line(commands, &fonts.ui, &report.location, text_primary(), 12.0);
    let bt_label = line(commands, &fonts.ui, "Backtrace:", text_muted(), 12.0);

    // Scrollable backtrace.
    let bt_content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        })
        .id();
    let bt_text = commands
        .spawn((
            Text::new(report.backtrace.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(bt_content).add_child(bt_text);
    let bt_scroll = scroll_area(commands, bt_content, 240.0);

    // Button row.
    let copy_btn = button(commands, &fonts.ui, "Copy to Clipboard");
    commands.entity(copy_btn).insert(CrashCopyButton);
    let close_btn = button(commands, &fonts.ui, "Close");
    commands.entity(close_btn).insert(CrashCloseButton);
    let btn_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    commands.entity(btn_row).add_children(&[copy_btn, close_btn]);

    commands.entity(panel).add_children(&[
        heading, ts, err_label, err, loc_label, loc, bt_label, bt_scroll, btn_row,
    ]);
    commands.entity(backdrop).add_child(panel);
}

/// Handle Copy / Close clicks.
pub(crate) fn crash_overlay_buttons(
    mut state: ResMut<CrashReportWindowState>,
    close_q: Query<&Interaction, (Changed<Interaction>, With<CrashCloseButton>)>,
    copy_q: Query<&Interaction, (Changed<Interaction>, With<CrashCopyButton>)>,
) {
    if close_q.iter().any(|i| *i == Interaction::Pressed) {
        state.show_window = false;
    }
    if copy_q.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(report) = state.report.clone() {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(report.format());
            }
        }
    }
}
