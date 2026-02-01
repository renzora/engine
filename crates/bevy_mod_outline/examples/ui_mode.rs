use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    render::RenderDebugFlags,
    state::state::FreelyMutableState,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins,
            OutlinePlugin,
            WireframePlugin {
                debug_flags: RenderDebugFlags::empty(),
            },
        ))
        .insert_state(DrawMethod::Extrude)
        .insert_state(DrawShape::Cone)
        .insert_state(DrawOrientation::Front)
        .init_resource::<Shapes>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                highlight::<DrawMethod>,
                highlight::<DrawShape>,
                highlight::<DrawOrientation>,
                interaction::<DrawMethod>,
                interaction::<DrawShape>,
                interaction::<DrawOrientation>,
                change_mode,
                change_shape,
                change_orientation,
                rotate_y,
            ),
        )
        .run();
}

#[derive(Component)]
struct TheObject;

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum DrawMethod {
    Extrude,
    ExtrudeDoubleSided,
    JumpFlood,
    JumpFloodDoubleSided,
}

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum DrawShape {
    Cone,
    Triangle,
    Rectangle,
    Circle,
}

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum DrawOrientation {
    Front,
    Back,
}

#[derive(Component)]
struct RotateY(f32);

#[derive(Resource)]
struct Shapes {
    cone: Handle<Mesh>,
    triangle: Handle<Mesh>,
    rectangle: Handle<Mesh>,
    circle: Handle<Mesh>,
}

impl Shapes {
    fn get(&self, shape: DrawShape) -> Handle<Mesh> {
        match shape {
            DrawShape::Cone => self.cone.clone(),
            DrawShape::Triangle => self.triangle.clone(),
            DrawShape::Rectangle => self.rectangle.clone(),
            DrawShape::Circle => self.circle.clone(),
        }
    }
}

impl FromWorld for Shapes {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.get_resource_mut::<Assets<Mesh>>().unwrap();
        let settings = GenerateOutlineNormalsSettings::default().with_stretch_edges(true);
        Self {
            cone: meshes.add(
                Cone::new(1.0, 1.0)
                    .mesh()
                    .build()
                    .rotated_by(Quat::from_rotation_x(FRAC_PI_2))
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
            triangle: meshes.add(
                Triangle2d::new(
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, -1.0),
                    Vec2::new(-1.0, -1.0),
                )
                .mesh()
                .build()
                .with_generated_outline_normals(&settings)
                .unwrap(),
            ),
            rectangle: meshes.add(
                Rectangle::new(2.0, 2.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
            circle: meshes.add(
                Circle::new(1.0)
                    .mesh()
                    .build()
                    .with_generated_outline_normals(&settings)
                    .unwrap(),
            ),
        }
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shapes: Res<Shapes>,
) {
    // Add shape
    commands.spawn((
        Mesh3d(shapes.triangle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        OutlineVolume {
            visible: true,
            width: 25.0,
            colour: Color::srgb(1.0, 1.0, 0.0),
        },
        Wireframe,
        TheObject,
    ));

    // Add buttons
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|parent| {
                    create_buttons(
                        parent,
                        &[
                            DrawMethod::Extrude,
                            DrawMethod::ExtrudeDoubleSided,
                            DrawMethod::JumpFlood,
                            DrawMethod::JumpFloodDoubleSided,
                        ],
                    );
                    create_buttons(
                        parent,
                        &[
                            DrawShape::Cone,
                            DrawShape::Triangle,
                            DrawShape::Rectangle,
                            DrawShape::Circle,
                        ],
                    );
                    create_buttons(parent, &[DrawOrientation::Front, DrawOrientation::Back]);
                });
        });

    // Add light source and camera
    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, 8.0, 4.0)));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
    ));
}

fn create_buttons<T: Component + States>(builder: &mut ChildSpawnerCommands, values: &[T]) {
    builder
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|parent| {
            for value in values {
                parent
                    .spawn((
                        Button,
                        Node {
                            margin: UiRect::all(Val::Px(5.0)),
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(5.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor(Color::BLACK),
                        BorderRadius::MAX,
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        value.clone(),
                    ))
                    .with_children(|parent| {
                        parent
                            .spawn(Text::new(format!("{:?}", value)))
                            .insert(TextColor(Color::WHITE));
                    });
            }
        });
}

fn highlight<T: Component + States>(
    mut query: Query<(&mut BorderColor, &T)>,
    state: Res<State<T>>,
) {
    for (mut border, m) in query.iter_mut() {
        *border = if m == state.get() {
            BorderColor(Color::srgb(0.0, 0.0, 1.0))
        } else {
            BorderColor(Color::BLACK)
        };
    }
}

fn interaction<T: Component + FreelyMutableState>(
    query: Query<(&Interaction, &T)>,
    mut armed: Local<bool>,
    mut state: ResMut<NextState<T>>,
) {
    let mut any_pressed = false;
    for (i, m) in query.iter() {
        if *i == Interaction::Pressed {
            any_pressed = true;
            if *armed {
                state.set(m.clone());
            }
        }
    }
    *armed = !any_pressed;
}

fn change_mode(
    mut commands: Commands,
    mut reader: EventReader<StateTransitionEvent<DrawMethod>>,
    query: Query<Entity, With<TheObject>>,
) {
    for event in reader.read() {
        if let Ok(entity) = query.single() {
            commands
                .entity(entity)
                .insert(match event.entered.unwrap() {
                    DrawMethod::Extrude => OutlineMode::ExtrudeFlat,
                    DrawMethod::ExtrudeDoubleSided => OutlineMode::ExtrudeFlatDoubleSided,
                    DrawMethod::JumpFlood => OutlineMode::FloodFlat,
                    DrawMethod::JumpFloodDoubleSided => OutlineMode::FloodFlatDoubleSided,
                });
        }
    }
}

fn change_shape(
    mut commands: Commands,
    mut reader: EventReader<StateTransitionEvent<DrawShape>>,
    query: Query<Entity, With<TheObject>>,
    shapes: Res<Shapes>,
) {
    for event in reader.read() {
        if let Ok(entity) = query.single() {
            commands
                .entity(entity)
                .insert(Mesh3d(shapes.get(event.entered.unwrap())));
        }
    }
}

fn change_orientation(
    mut commands: Commands,
    mut reader: EventReader<StateTransitionEvent<DrawOrientation>>,
    query: Query<Entity, With<TheObject>>,
) {
    for event in reader.read() {
        if let Ok(entity) = query.single() {
            commands
                .entity(entity)
                .insert(RotateY(match event.entered.unwrap() {
                    DrawOrientation::Front => 0.0,
                    DrawOrientation::Back => PI,
                }));
        }
    }
}

fn rotate_y(
    mut commands: Commands,
    mut query: Query<(Entity, &RotateY, &mut Transform)>,
    time: Res<Time>,
) {
    for (entity, target, mut transform) in query.iter_mut() {
        let current_angle = transform.rotation.to_euler(EulerRot::YXZ).0;
        let target_angle = target.0;

        let delta = target_angle - current_angle;
        let max_step = 2.0 * time.delta_secs();
        let step_size = max_step.min(delta.abs());

        transform.rotation = Quat::from_rotation_y(if step_size < max_step {
            // Target reached, remove the RotateY component
            commands.entity(entity).remove::<RotateY>();
            target_angle
        } else {
            // Animate towards target
            current_angle + step_size * delta.signum()
        });
    }
}
