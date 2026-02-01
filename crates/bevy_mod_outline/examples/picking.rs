use bevy::prelude::*;
use bevy_mod_outline::*;

/// Tag to track if an object is selected or not
#[derive(Component)]
struct Selected;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_selected)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut window: Query<Entity, With<Window>>,
) {
    let shapes = [
        meshes.add(Cuboid::default()),
        meshes.add(Torus::default()),
        meshes.add(Capsule3d::default()),
        meshes.add(Tetrahedron::default()),
    ];
    let material = [
        materials.add(StandardMaterial::from_color(Color::srgb_u8(255, 0, 0))),
        materials.add(StandardMaterial::from_color(Color::srgb_u8(255, 255, 0))),
        materials.add(StandardMaterial::from_color(Color::srgb_u8(0, 255, 0))),
        materials.add(StandardMaterial::from_color(Color::srgb_u8(0, 0, 255))),
    ];
    const DISTANCE: f32 = 1.5;
    let positions = [
        Vec3::new(-DISTANCE, DISTANCE, DISTANCE),
        Vec3::new(DISTANCE, DISTANCE, DISTANCE),
        Vec3::new(-DISTANCE, -DISTANCE, DISTANCE),
        Vec3::new(DISTANCE, -DISTANCE, DISTANCE),
    ];

    // Spawn shapes with outline and picking observer
    for i in 0..shapes.len() {
        commands
            .spawn((
                Mesh3d(shapes[i].clone()),
                MeshMaterial3d(material[i].clone()),
                Transform::from_translation(positions[i]),
                OutlineVolume {
                    width: 5.0f32,
                    ..default()
                },
                OutlineMode::FloodFlat,
                Pickable::default(),
            ))
            .observe(on_click);
    }

    // Add an observer to the window to respond to clicks that don't hit a mesh
    if let Ok(entity) = window.single_mut() {
        if let Ok(mut window) = commands.get_entity(entity) {
            window.observe(on_click);
        }
    }

    // Add ground
    commands.spawn((
        Mesh3d(shapes[0].clone()), // Reuse cuboid handle
        MeshMaterial3d(materials.add(StandardMaterial::from_color(Color::WHITE))),
        Transform::from_translation(Vec3::new(0.0, -5.0, 0.0))
            .with_scale(Vec3::new(15.0, 1.0, 15.0)),
    ));

    // Add a light source
    commands.spawn((
        PointLight {
            color: Color::srgb_u8(255, 255, 192),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Add camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Observer system that manages what pickable objects are selected
fn on_click(
    mut event: Trigger<Pointer<Click>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut OutlineVolume, Option<&Selected>)>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // Disable propagation to prevent observers other than the first in the queue from responding
    event.propagate(false);

    /// Helper function to deselect every selected entity
    fn deselect_all(
        commands: &mut Commands,
        query: &mut Query<(Entity, &mut OutlineVolume, Option<&Selected>)>,
    ) {
        for (entity, mut outline, selected) in query.iter_mut() {
            if selected.is_some() {
                if let Ok(mut entity) = commands.get_entity(entity) {
                    entity.remove::<Selected>();
                    outline.visible = false;
                }
            }
        }
    }

    // Check if the user wants to select multiple objects
    let multi_select = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // Deselect everything if the user does not want to select multiple objects
    if !multi_select {
        deselect_all(&mut commands, &mut query);
    }

    // Select a mesh
    if let Ok((entity, mut outline, selected)) = query.get_mut(event.target) {
        if let Ok(mut entity) = commands.get_entity(entity) {
            // When selecting multiple objects, allow clicking a selected object to deselect it.
            if multi_select && selected.is_some() {
                entity.remove::<Selected>();
                outline.visible = false;
            } else {
                entity.insert(Selected);
                outline.visible = true;
            }
        }
    }
}

/// Rotate selected meshes
fn rotate_selected(mut query: Query<&mut Transform, With<Selected>>) {
    const SPEED: f32 = 0.1;
    for mut transform in query.iter_mut() {
        transform.rotate_x(SPEED);
        transform.rotate_y(SPEED * 1.5);
        transform.rotate_z(SPEED * 0.5);
    }
}
