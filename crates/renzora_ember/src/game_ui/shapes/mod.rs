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
pub mod tri3;
pub mod triangle;
pub mod wedge;

pub use arc::*;
pub use circle::*;
pub use line::*;
pub use polygon::*;
pub use radial_progress::*;
pub use rectangle::*;
pub use tri3::*;
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
        // Every shape type stays registered, so a scene naming one still loads
        // in a build without it (it just draws nothing). What each shape's
        // feature decides is whether its material is: a `UiMaterialPlugin` is a
        // whole render pipeline, about 200 KiB of code per shape, which is why
        // a lean export keeps only the shapes the project uses.
        app.register_type::<UiShapeWidget>();
        app.register_type::<CircleShape>();
        app.register_type::<ArcShape>();
        app.register_type::<RadialProgressShape>();
        app.register_type::<LineShape>();
        app.register_type::<TriangleShape>();
        app.register_type::<PolygonShape>();
        app.register_type::<RectangleShape>();
        app.register_type::<WedgeShape>();

        #[cfg(feature = "shape_circle")]
        {
            bevy::asset::embedded_asset!(app, "circle.wesl");
            add_shape::<CircleMaterial, _>(app, circle::sync_circle_materials);
        }
        #[cfg(feature = "shape_arc")]
        {
            bevy::asset::embedded_asset!(app, "arc.wesl");
            add_shape::<ArcMaterial, _>(app, arc::sync_arc_materials);
        }
        #[cfg(feature = "shape_radial_progress")]
        {
            bevy::asset::embedded_asset!(app, "radial_progress.wesl");
            add_shape::<RadialProgressMaterial, _>(app, radial_progress::sync_radial_progress_materials);
        }
        #[cfg(feature = "shape_line")]
        {
            bevy::asset::embedded_asset!(app, "line.wesl");
            add_shape::<LineMaterial, _>(app, line::sync_line_materials);
        }
        #[cfg(feature = "shape_triangle")]
        {
            bevy::asset::embedded_asset!(app, "triangle.wesl");
            add_shape::<TriangleMaterial, _>(app, triangle::sync_triangle_materials);
        }
        #[cfg(feature = "shape_polygon")]
        {
            bevy::asset::embedded_asset!(app, "polygon.wesl");
            add_shape::<PolygonMaterial, _>(app, polygon::sync_polygon_materials);
        }
        #[cfg(feature = "shape_rectangle")]
        {
            bevy::asset::embedded_asset!(app, "rectangle.wesl");
            add_shape::<RectangleMaterial, _>(app, rectangle::sync_rectangle_materials);
        }
        #[cfg(feature = "shape_wedge")]
        {
            bevy::asset::embedded_asset!(app, "wedge.wesl");
            add_shape::<WedgeMaterial, _>(app, wedge::sync_wedge_materials);
        }
        #[cfg(feature = "shape_tri3")]
        {
            bevy::asset::embedded_asset!(app, "tri3.wesl");
            add_shape::<Tri3Material, _>(app, tri3::sync_tri3_materials);
        }
    }
}

/// A shape's material, built from its component.
pub trait ShapeMaterial: UiMaterial {
    type Shape: Component;
    fn from_shape(shape: &Self::Shape) -> Self;
}

macro_rules! shape_material {
    ($($mat:ty => $shape:ty),* $(,)?) => {$(
        impl ShapeMaterial for $mat {
            type Shape = $shape;
            fn from_shape(shape: &$shape) -> Self {
                <$mat>::from_shape(shape)
            }
        }
    )*};
}
shape_material!(
    CircleMaterial => CircleShape,
    ArcMaterial => ArcShape,
    RadialProgressMaterial => RadialProgressShape,
    LineMaterial => LineShape,
    TriangleMaterial => TriangleShape,
    PolygonMaterial => PolygonShape,
    RectangleMaterial => RectangleShape,
    WedgeMaterial => WedgeShape,
    Tri3Material => Tri3Shape,
);

/// Install one shape's material pipeline, its repaint system and its
/// rehydration. Called only for the shapes this build includes.
#[allow(dead_code)]
fn add_shape<M, Marker>(app: &mut App, sync: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, Marker>)
where
    M: ShapeMaterial,
    M::Data: PartialEq + Eq + std::hash::Hash + Clone,
{
    app.add_plugins(UiMaterialPlugin::<M>::default());
    app.add_systems(Update, (sync, rehydrate_shape_material::<M>));
}

/// Rehydrates a shape's `MaterialNode` after scene deserialization (or a fresh
/// spawn that brought only the component). Shape components survive
/// serialization; `MaterialNode<T>` handles don't.
#[allow(dead_code)]
fn rehydrate_shape_material<M: ShapeMaterial>(
    mut commands: Commands,
    shapes: Query<(Entity, &M::Shape), (Added<M::Shape>, Without<MaterialNode<M>>)>,
    mut materials: ResMut<Assets<M>>,
) {
    for (entity, shape) in &shapes {
        let handle = materials.add(M::from_shape(shape));
        commands.entity(entity).try_insert(MaterialNode(handle));
    }
}
