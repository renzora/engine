use super::{ComputeMassProperties2d, MassProperties2d};
use bevy_math::{
    ops,
    primitives::{
        Annulus, Arc2d, Capsule2d, Circle, CircularSector, CircularSegment, ConvexPolygon, Ellipse,
        Line2d, Measured2d, Plane2d, Polyline2d, Rectangle, RegularPolygon, Rhombus, Segment2d,
        Triangle2d,
    },
    FloatPow, Vec2,
};

impl ComputeMassProperties2d for Circle {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        self.radius.squared() / 2.0
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for CircularSector {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        0.5 * ops::powf(self.radius(), 4.0) * self.angle()
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        let angle = self.angle();
        let y = 2.0 * self.radius() * ops::sin(angle) / (3.0 * angle);
        Vec2::new(0.0, y)
    }
}

impl ComputeMassProperties2d for CircularSegment {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        let angle = self.angle();
        let (sin, cos) = ops::sin_cos(angle);
        ops::powf(self.radius(), 4.0) / 4.0 * (angle - sin + 2.0 / 3.0 * sin * (1.0 - cos) / 2.0)
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        let y = self.radius() * ops::sin(self.half_angle()).cubed()
            / (6.0 * self.half_angle() - ops::sin(self.angle()));
        Vec2::new(0.0, y)
    }
}

impl ComputeMassProperties2d for Ellipse {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        self.half_size.length_squared() / 4.0
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for Annulus {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        0.5 * (self.outer_circle.radius.squared() + self.inner_circle.radius.squared())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for Triangle2d {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        // Adapted from Box2D: https://github.com/erincatto/box2d/blob/411acc32eb6d4f2e96fc70ddbdf01fe5f9b16230/src/collision/b2_polygon_shape.cpp#L274

        // Note: The center of mass is used here, unlike in Box2D's or Parry's version.
        let center_of_mass = self.center_of_mass();
        let com_a = self.vertices[1] - center_of_mass;
        let com_c = self.vertices[2] - center_of_mass;

        (com_a.length_squared() + com_a.dot(com_c) + com_c.length_squared()) / 6.0
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        (self.vertices[0] + self.vertices[1] + self.vertices[2]) / 3.0
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties2d {
        let area = self.area();
        let center_of_mass = self.center_of_mass();

        if area < f32::EPSILON {
            return MassProperties2d::new(0.0, 0.0, center_of_mass);
        }

        let mass = area * density;

        MassProperties2d::new(mass, self.angular_inertia(mass), center_of_mass)
    }
}

impl ComputeMassProperties2d for Rectangle {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        self.half_size.length_squared() / 3.0
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for Rhombus {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        self.half_diagonals.length_squared() / 12.0
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for RegularPolygon {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.area() * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        let half_external_angle = core::f32::consts::PI / self.sides as f32;
        self.circumradius().squared() / 6.0 * (1.0 + 2.0 * ops::cos(half_external_angle).squared())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }
}

impl ComputeMassProperties2d for Capsule2d {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        let area = self.radius * (core::f32::consts::PI * self.radius + 4.0 * self.half_length);
        area * density
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        // The rectangle and hemicircle parts
        let rectangle = Rectangle {
            half_size: Vec2::new(self.radius, self.half_length),
        };
        let rectangle_height = rectangle.half_size.y * 2.0;
        let circle = Circle::new(self.radius);

        // Areas
        let rectangle_area = rectangle.area();
        let circle_area = circle.area();

        // Masses
        let density = 1.0 / (rectangle_area + circle_area);
        let rectangle_mass = rectangle_area * density;
        let circle_mass = circle_area * density;

        // Principal inertias
        let rectangle_inertia = rectangle.angular_inertia(rectangle_mass);
        let circle_inertia = circle.angular_inertia(circle_mass);

        // Total inertia
        let mut capsule_inertia = rectangle_inertia + circle_inertia;

        // Compensate for the hemicircles being away from the rotation axis using the parallel axis theorem.
        capsule_inertia += (rectangle_height.squared() * 0.25
            + rectangle_height * self.radius * 3.0 / 8.0)
            * circle_mass;

        capsule_inertia
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        Vec2::ZERO
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties2d {
        // The rectangle and hemicircle parts
        let rectangle = Rectangle {
            half_size: Vec2::new(self.radius, self.half_length),
        };
        let rectangle_height = rectangle.half_size.y * 2.0;
        let circle = Circle::new(self.radius);

        // Areas
        let rectangle_area = rectangle.area();
        let circle_area = circle.area();

        // Masses
        let rectangle_mass = rectangle_area * density;
        let circle_mass = circle_area * density;

        // Principal inertias
        let rectangle_inertia = rectangle.angular_inertia(rectangle_mass);
        let circle_inertia = circle.angular_inertia(circle_mass);

        // Total inertia
        let mut capsule_inertia = rectangle_inertia + circle_inertia;

        // Compensate for the hemicircles being away from the rotation axis using the parallel axis theorem.
        capsule_inertia += (rectangle_height.squared() * 0.25
            + rectangle_height * self.radius * 3.0 / 8.0)
            * circle_mass;

        MassProperties2d::new(rectangle_mass + circle_mass, capsule_inertia, Vec2::ZERO)
    }
}

impl ComputeMassProperties2d for ConvexPolygon {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        convex_polygon_mass(self.vertices(), density)
    }

    #[inline]
    fn unit_angular_inertia(&self) -> f32 {
        convex_polygon_unit_angular_inertia(self.vertices())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec2 {
        convex_polygon_area_and_center_of_mass(self.vertices()).1
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties2d {
        convex_polygon_mass_properties(self.vertices(), density)
    }
}

/// Computes the mass of a convex polygon defined by its vertices and density.
///
/// No checks are performed to ensure the polygon is convex.
#[inline]
pub fn convex_polygon_mass(vertices: &[Vec2], density: f32) -> f32 {
    let geometric_center =
        vertices.iter().fold(Vec2::ZERO, |acc, vtx| acc + *vtx) / vertices.len() as f32;

    // Initialize polygon area.
    let mut area = 0.0;

    // Create a peekable iterator over the polygon vertices.
    let mut iter = vertices.iter().peekable();
    let Some(first) = iter.peek().copied().copied() else {
        return 0.0;
    };

    // Iterate through vertices, computing the sum of the areas of triangles.
    // Each triangle is formed by the current vertex, next vertex, and the geometric center of the polygon.
    while let Some(vertex) = iter.next() {
        let (a, b, c) = (
            *vertex,
            iter.peek().copied().copied().unwrap_or(first),
            geometric_center,
        );
        let tri_area = Triangle2d::new(a, b, c).area();
        area += tri_area;
    }

    area * density
}

/// Computes the unit angular inertia of a convex polygon defined by its vertices.
///
/// No checks are performed to ensure the polygon is convex.
#[inline]
pub fn convex_polygon_unit_angular_inertia(vertices: &[Vec2]) -> f32 {
    // The polygon is assumed to be convex.
    let (area, center_of_mass) = convex_polygon_area_and_center_of_mass(vertices);

    if area < f32::EPSILON {
        return 0.0;
    }

    // Initialize polygon inertia.
    let mut inertia = 0.0;

    // Create a peekable iterator over the polygon vertices.
    let mut iter = vertices.iter().peekable();
    let first = **iter.peek().unwrap();

    // Iterate through vertices, computing the sum of the areas of triangles.
    // Each triangle is formed by the current vertex, next vertex, and the geometric center of the polygon.
    while let Some(vertex) = iter.next() {
        let triangle = Triangle2d::new(
            *vertex,
            iter.peek().copied().copied().unwrap_or(first),
            center_of_mass,
        );
        inertia += triangle.unit_angular_inertia() * triangle.area();
    }

    inertia / area
}

/// Computes the mass properties of a convex polygon defined by its vertices and density.
///
/// No checks are performed to ensure the polygon is convex.
#[inline]
pub fn convex_polygon_mass_properties(vertices: &[Vec2], density: f32) -> MassProperties2d {
    // The polygon is assumed to be convex.
    let (area, center_of_mass) = convex_polygon_area_and_center_of_mass(vertices);

    if area < f32::EPSILON {
        return MassProperties2d::new(0.0, 0.0, center_of_mass);
    }

    // Initialize polygon inertia.
    let mut inertia = 0.0;

    // Create a peekable iterator over the polygon vertices.
    let mut iter = vertices.iter().peekable();
    let first = **iter.peek().unwrap();

    // Iterate through vertices, computing the sum of the areas of triangles.
    // Each triangle is formed by the current vertex, next vertex, and the geometric center of the polygon.
    while let Some(vertex) = iter.next() {
        let triangle = Triangle2d::new(
            *vertex,
            iter.peek().copied().copied().unwrap_or(first),
            center_of_mass,
        );
        inertia += triangle.unit_angular_inertia() * triangle.area();
    }

    MassProperties2d::new(area * density, inertia * density, center_of_mass)
}

/// Computes the area and center of mass of a convex polygon defined by its vertices.
///
/// No checks are performed to ensure the polygon is convex.
#[inline]
pub fn convex_polygon_area_and_center_of_mass(vertices: &[Vec2]) -> (f32, Vec2) {
    let geometric_center =
        vertices.iter().fold(Vec2::ZERO, |acc, vtx| acc + *vtx) / vertices.len() as f32;

    // Initialize polygon area and center.
    let mut area = 0.0;
    let mut center = Vec2::ZERO;

    // Create a peekable iterator over the polygon vertices.
    let mut iter = vertices.iter().peekable();
    let Some(first) = iter.peek().copied().copied() else {
        return (0.0, Vec2::ZERO);
    };

    // Iterate through vertices, computing the sum of the areas and centers of triangles.
    // Each triangle is formed by the current vertex, next vertex, and the geometric center of the polygon.
    while let Some(vertex) = iter.next() {
        let (a, b, c) = (
            *vertex,
            iter.peek().copied().copied().unwrap_or(first),
            geometric_center,
        );
        let tri_area = Triangle2d::new(a, b, c).area();
        let tri_center = (a + b + c) / 3.0;

        area += tri_area;
        center += tri_center * tri_area;
    }

    if area < f32::EPSILON {
        (area, geometric_center)
    } else {
        (area, center / area)
    }
}

macro_rules! impl_zero_mass_properties_2d {
    ($($shape:ty),*) => {
        $(
            impl ComputeMassProperties2d for $shape {
                #[inline]
                fn mass(&self, _density: f32) -> f32 {
                    0.0
                }

                #[inline]
                fn unit_angular_inertia(&self) -> f32 {
                    0.0
                }

                #[inline]
                fn angular_inertia(&self, _mass: f32) -> f32 {
                    0.0
                }

                #[inline]
                fn center_of_mass(&self) -> Vec2 {
                    Vec2::ZERO
                }

                #[inline]
                fn mass_properties(&self, _density: f32) -> MassProperties2d {
                    MassProperties2d::ZERO
                }
            }
        )*
    };
}

impl_zero_mass_properties_2d!(Arc2d);
impl_zero_mass_properties_2d!(Plane2d);
impl_zero_mass_properties_2d!(Line2d);
impl_zero_mass_properties_2d!(Segment2d);
impl_zero_mass_properties_2d!(Polyline2d);

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use approx::assert_relative_eq;
    use bevy_math::ShapeSample;
    use rand::SeedableRng;

    use super::*;

    macro_rules! test_shape {
        ($test_name:tt, $shape:expr) => {
            #[test]
            fn $test_name() {
                let shape = $shape;

                // Sample enough points to have a close enough point cloud representation of the shape.
                let mut rng = rand_chacha::ChaCha8Rng::from_seed(Default::default());
                let points = (0..1_000_000)
                    .map(|_| shape.sample_interior(&mut rng))
                    .collect::<Vec<_>>();

                // Compute the mass properties to test.
                let density = 2.0;
                let mass = shape.mass(density);
                let angular_inertia = shape.angular_inertia(mass);
                let center_of_mass = shape.center_of_mass();

                // First, test that the individually computed properties match the full properties.
                let mass_props = shape.mass_properties(density);
                assert_relative_eq!(mass, mass_props.mass);
                assert_relative_eq!(angular_inertia, mass_props.angular_inertia);
                assert_relative_eq!(center_of_mass, mass_props.center_of_mass);

                // Estimate the expected mass properties using the point cloud.
                // Note: We could also approximate the mass using Monte Carlo integration.
                //       This would require point containment checks.
                let expected = MassProperties2d::from_point_cloud(&points, mass);

                assert_relative_eq!(mass, expected.mass);
                assert_relative_eq!(angular_inertia, expected.angular_inertia, epsilon = 0.1);
                assert_relative_eq!(center_of_mass, expected.center_of_mass, epsilon = 0.01);
            }
        };
    }

    // TODO: Test randomized shape definitions.

    test_shape!(circle, Circle::new(2.0));
    // test_shape!(circular_sector, CircularSector::new(2.0, TAU));
    // test_shape!(circular_segment, CircularSegment::new(2.0, TAU));
    // test_shape!(ellipse, Ellipse::new(2.0, 1.0));
    test_shape!(annulus, Annulus::new(1.0, 2.0));
    test_shape!(
        triangle,
        Triangle2d::new(
            Vec2::new(8.0, 6.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(6.0, 2.0)
        )
    );
    test_shape!(rectangle, Rectangle::new(2.0, 1.0));
    // test_shape!(rhombus, Rhombus::new(2.0, 1.0));
    // test_shape!(regular_polygon, RegularPolygon::new(2.0, 6));
    // test_shape!(polygon, Polygon::new([Vec2::ZERO, Vec2::X, Vec2::Y]));
    test_shape!(capsule, Capsule2d::new(1.0, 0.25));
}
