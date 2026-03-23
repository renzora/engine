use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    log,
    prelude::*,
};

pub struct CameraPlugin;

#[derive(Debug, Component)]
pub struct OrbitController;

#[derive(Debug, Component)]
pub struct CameraController;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(PostUpdate, (handle_rotation, handle_zoom));
        log::info!("Camera Plugin loaded");
    }
}

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            Transform::from_rotation(Quat::from_rotation_z(-1.0)),
            Visibility::default(),
            OrbitController,
        ))
        .with_children(|b| {
            b.spawn((
                Camera3d::default(),
                Transform::from_xyz(0.0, 30.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                CameraController,
            ));
        });
}

pub fn handle_rotation(
    mut cam_controls: Query<&mut Transform, With<OrbitController>>,
    mut motion_evr: MessageReader<MouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();
    let mut transform = cam_controls.single_mut().unwrap();
    if buttons.pressed(MouseButton::Left) {
        for ev in motion_evr.read() {
            let delta = ev.delta * delta_time * 0.1;
            transform.rotate_y(-delta.x);
            transform.rotate_local_z(delta.y);
        }
    }
}

pub fn handle_zoom(
    mut cam_controls: Query<&mut Transform, With<CameraController>>,
    mut scroll_evr: MessageReader<MouseWheel>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();
    let mut transform = cam_controls.single_mut().unwrap();
    let forward = transform.forward();
    for ev in scroll_evr.read() {
        transform.translation += forward * ev.y * delta_time * 10.0;
    }
}
