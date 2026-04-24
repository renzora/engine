//! SDF-based shape primitives rendered via `UiMaterial`.
//!
//! Each shape has:
//! - A **component** (`CircleShape`, `ArcShape`, etc.) with logical parameters
//! - A **material** implementing `UiMaterial` with a WGSL SDF shader
//! - A **sync system** that updates the material when the component changes
//!
//! Shapes are spawned as regular UI entities with `Node` for layout sizing
//! and `MaterialNode<T>` for rendering.

pub mod arc;
pub mod circle;
pub mod line;
pub mod polygon;
pub mod radial_progress;
pub mod rectangle;
pub mod triangle;
pub mod wedge;

pub use arc::*;
pub use circle::*;
pub use line::*;
pub use polygon::*;
pub use radial_progress::*;
pub use rectangle::*;
pub use triangle::*;
pub use wedge::*;

use bevy::prelude::*;

/// Marker component for shape widget entities.
/// Used to exclude shapes from `BackgroundColor` insertion in `ensure_style_components`.
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct UiShapeWidget;

pub struct ShapesPlugin;

impl Plugin for ShapesPlugin {
    fn build(&self, app: &mut App) {
        // Register embedded shaders
        bevy::asset::embedded_asset!(app, "circle.wgsl");
        bevy::asset::embedded_asset!(app, "arc.wgsl");
        bevy::asset::embedded_asset!(app, "radial_progress.wgsl");
        bevy::asset::embedded_asset!(app, "line.wgsl");
        bevy::asset::embedded_asset!(app, "triangle.wgsl");
        bevy::asset::embedded_asset!(app, "polygon.wgsl");
        bevy::asset::embedded_asset!(app, "rectangle.wgsl");
        bevy::asset::embedded_asset!(app, "wedge.wgsl");

        app.add_plugins((
            UiMaterialPlugin::<CircleMaterial>::default(),
            UiMaterialPlugin::<ArcMaterial>::default(),
            UiMaterialPlugin::<RadialProgressMaterial>::default(),
            UiMaterialPlugin::<LineMaterial>::default(),
            UiMaterialPlugin::<TriangleMaterial>::default(),
            UiMaterialPlugin::<PolygonMaterial>::default(),
            UiMaterialPlugin::<RectangleMaterial>::default(),
            UiMaterialPlugin::<WedgeMaterial>::default(),
        ));

        app.register_type::<UiShapeWidget>();
        app.register_type::<CircleShape>();
        app.register_type::<ArcShape>();
        app.register_type::<RadialProgressShape>();
        app.register_type::<LineShape>();
        app.register_type::<TriangleShape>();
        app.register_type::<PolygonShape>();
        app.register_type::<RectangleShape>();
        app.register_type::<WedgeShape>();

        app.add_systems(
            Update,
            (
                circle::sync_circle_materials,
                arc::sync_arc_materials,
                radial_progress::sync_radial_progress_materials,
                line::sync_line_materials,
                triangle::sync_triangle_materials,
                polygon::sync_polygon_materials,
                rectangle::sync_rectangle_materials,
                wedge::sync_wedge_materials,
            ),
        );
        app.add_systems(
            Update,
            (rehydrate_basic_shape_materials, rehydrate_polygonal_shape_materials),
        );
    }
}

/// Rehydrates `MaterialNode` handles after scene deserialization.
/// Shape components survive serialization but `MaterialNode<T>` handles don't.
// Bevy's system fn parameter limit is 16. The combined rehydrate query uses
// 17 (1 commands + 8 queries + 8 asset resources), so we split the work.

fn rehydrate_basic_shape_materials(
    mut commands: Commands,
    circles: Query<(Entity, &CircleShape), (Added<CircleShape>, Without<MaterialNode<CircleMaterial>>)>,
    arcs: Query<(Entity, &ArcShape), (Added<ArcShape>, Without<MaterialNode<ArcMaterial>>)>,
    radials: Query<(Entity, &RadialProgressShape), (Added<RadialProgressShape>, Without<MaterialNode<RadialProgressMaterial>>)>,
    lines: Query<(Entity, &LineShape), (Added<LineShape>, Without<MaterialNode<LineMaterial>>)>,
    mut circle_mats: ResMut<Assets<CircleMaterial>>,
    mut arc_mats: ResMut<Assets<ArcMaterial>>,
    mut radial_mats: ResMut<Assets<RadialProgressMaterial>>,
    mut line_mats: ResMut<Assets<LineMaterial>>,
) {
    for (entity, shape) in &circles {
        let handle = circle_mats.add(CircleMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &arcs {
        let handle = arc_mats.add(ArcMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &radials {
        let handle = radial_mats.add(RadialProgressMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &lines {
        let handle = line_mats.add(LineMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
}

fn rehydrate_polygonal_shape_materials(
    mut commands: Commands,
    triangles: Query<(Entity, &TriangleShape), (Added<TriangleShape>, Without<MaterialNode<TriangleMaterial>>)>,
    polygons: Query<(Entity, &PolygonShape), (Added<PolygonShape>, Without<MaterialNode<PolygonMaterial>>)>,
    rectangles: Query<(Entity, &RectangleShape), (Added<RectangleShape>, Without<MaterialNode<RectangleMaterial>>)>,
    wedges: Query<(Entity, &WedgeShape), (Added<WedgeShape>, Without<MaterialNode<WedgeMaterial>>)>,
    mut tri_mats: ResMut<Assets<TriangleMaterial>>,
    mut poly_mats: ResMut<Assets<PolygonMaterial>>,
    mut rect_mats: ResMut<Assets<RectangleMaterial>>,
    mut wedge_mats: ResMut<Assets<WedgeMaterial>>,
) {
    for (entity, shape) in &triangles {
        let handle = tri_mats.add(TriangleMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &polygons {
        let handle = poly_mats.add(PolygonMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &rectangles {
        let handle = rect_mats.add(RectangleMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
    for (entity, shape) in &wedges {
        let handle = wedge_mats.add(WedgeMaterial::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
}
