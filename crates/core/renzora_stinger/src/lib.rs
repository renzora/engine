use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StingerState {
    #[default]
    Stinger,
    Game,
}

pub struct StingerPlugin;

impl Plugin for StingerPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] StingerPlugin");
        app.init_state::<StingerState>();

        // Only show the stinger splash in standalone/runtime mode
        #[cfg(not(feature = "editor"))]
        {
            app.add_systems(Startup, setup_stinger)
                .add_systems(Update, tick_stinger.run_if(in_state(StingerState::Stinger)))
                .add_systems(OnExit(StingerState::Stinger), cleanup_stinger);
        }

        // In editor mode, skip straight to Game state
        #[cfg(feature = "editor")]
        app.add_systems(Startup, |mut next_state: ResMut<NextState<StingerState>>| {
            next_state.set(StingerState::Game);
        });
    }
}

#[derive(Component)]
struct StingerMarker;

#[derive(Resource)]
struct StingerTimer(Timer);

fn setup_stinger(mut commands: Commands) {
    commands.insert_resource(StingerTimer(Timer::from_seconds(3.0, TimerMode::Once)));

    commands.spawn((Camera2d, StingerMarker));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(i32::MAX),
            StingerMarker,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("renzora engine"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn tick_stinger(
    time: Res<Time>,
    mut timer: ResMut<StingerTimer>,
    mut next_state: ResMut<NextState<StingerState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    timer.0.tick(time.delta());

    let skip = keyboard.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some();

    if timer.0.fraction() >= 1.0 || skip {
        next_state.set(StingerState::Game);
    }
}

fn cleanup_stinger(mut commands: Commands, query: Query<Entity, With<StingerMarker>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
