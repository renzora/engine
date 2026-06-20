use super::{AngularInertiaTensor, ComputeMassProperties3d, MassProperties3d};
use bevy_math::{
    ops,
    prelude::Tetrahedron,
    primitives::{
        Capsule3d, Cone, ConicalFrustum, Cuboid, Cylinder, Line3d, Measured3d, Plane3d, Polyline3d,
        Segment3d, Sphere, Torus, Triangle3d,
    },
    FloatPow, Quat, Vec3,
};
use glam_matrix_extras::SymmetricMat3;

impl ComputeMassProperties3d for Sphere {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        Vec3::splat(0.4 * self.radius.squared())
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl ComputeMassProperties3d for Cuboid {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        let ix = self.half_size.x.squared() / 3.0;
        let iy = self.half_size.y.squared() / 3.0;
        let iz = self.half_size.z.squared() / 3.0;
        Vec3::new(iy + iz, ix + iz, ix + iy)
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl ComputeMassProperties3d for Cylinder {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        let radius_squared = self.radius.squared();
        let height_squared = self.half_height.squared() * 4.0;
        let principal = radius_squared / 2.0;
        let off_principal = (radius_squared * 3.0 + height_squared) / 12.0;
        Vec3::new(off_principal, principal, off_principal)
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl ComputeMassProperties3d for Capsule3d {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        // The cylinder and hemisphere parts
        let cylinder = Cylinder {
            radius: self.radius,
            half_height: self.half_length,
        };
        let cylinder_length = self.half_length * 2.0;
        let sphere = Sphere::new(self.radius);

        // Volumes
        let cylinder_volume = cylinder.volume();
        let sphere_volume = sphere.volume();

        // Masses
        let density = 1.0 / (cylinder_volume + sphere_volume);
        let cylinder_mass = cylinder_volume * density;
        let sphere_mass = sphere_volume * density;

        // Principal inertias
        let cylinder_inertia = cylinder.principal_angular_inertia(cylinder_mass);
        let sphere_inertia = sphere.principal_angular_inertia(sphere_mass);

        // Total inertia
        let mut capsule_inertia = cylinder_inertia + sphere_inertia;

        // Compensate for the hemispheres being away from the rotation axis using the parallel axis theorem.
        let extra = (cylinder_length.squared() * 0.25 + cylinder_length * self.radius * 3.0 / 8.0)
            * sphere_mass;
        capsule_inertia.x += extra;
        capsule_inertia.z += extra;

        capsule_inertia
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties3d {
        // The cylinder and hemisphere parts
        let cylinder = Cylinder {
            radius: self.radius,
            half_height: self.half_length,
        };
        let cylinder_length = self.half_length * 2.0;
        let sphere = Sphere::new(self.radius);

        // Volumes
        let cylinder_volume = cylinder.volume();
        let sphere_volume = sphere.volume();

        // Masses
        let cylinder_mass = cylinder_volume * density;
        let sphere_mass = sphere_volume * density;

        // Principal inertias
        let cylinder_inertia = cylinder.principal_angular_inertia(cylinder_mass);
        let sphere_inertia = sphere.principal_angular_inertia(sphere_mass);

        // Total inertia
        let mut capsule_inertia = cylinder_inertia + sphere_inertia;

        // Compensate for the hemispheres being away from the rotation axis using the parallel axis theorem.
        let extra = (cylinder_length.squared() * 0.25 + cylinder_length * self.radius * 3.0 / 8.0)
            * sphere_mass;
        capsule_inertia.x += extra;
        capsule_inertia.z += extra;

        MassProperties3d::new(cylinder_mass + sphere_mass, capsule_inertia, Vec3::ZERO)
    }
}

impl ComputeMassProperties3d for Cone {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        let radius_squared = self.radius.squared();
        let height_squared = self.height.squared();

        // About the Y axis
        let principal = 3.0 / 10.0 * radius_squared;

        // About the X or Z axis passing through the center of mass
        let off_principal = principal * 0.5 + 3.0 / 80.0 * height_squared;

        Vec3::new(off_principal, principal, off_principal)
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::new(0.0, -self.height * 0.25, 0.0)
    }
}

impl ComputeMassProperties3d for ConicalFrustum {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        if self.radius_top == self.radius_bottom {
            Cylinder::new(self.radius_top, self.height).mass(density)
        } else {
            // https://mathworld.wolfram.com/ConicalFrustum.html
            let radii_squared = self.radius_top.squared() + self.radius_bottom.squared();
            let volume = core::f32::consts::FRAC_PI_3
                * self.height
                * (radii_squared + self.radius_top * self.radius_bottom);
            volume * density
        }
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        if self.radius_top == self.radius_bottom {
            Cylinder::new(self.radius_top, self.height).unit_principal_angular_inertia()
        } else {
            let (min_radius, max_radius) = if self.radius_top < self.radius_bottom {
                (self.radius_top, self.radius_bottom)
            } else {
                (self.radius_bottom, self.radius_top)
            };

            // TODO: The principal angular inertia along the symmetry axis Y is straightforward to compute directly.
            //       However, I have not found a correct formula for the principal angular inertia along the other axes.
            //       It would be much more efficient to compute directly instead of subtracting cones like what we're doing here though.
            // let principal_y = 3.0 * (max_radius.powi(5) - min_radius.powi(5))
            //     / (10.0 * (max_radius.powi(3) - min_radius.powi(3)));

            // The conical frustum can be thought of as a cone with a smaller cone subtracted from it.
            // To get its angular inertia, we can subtract the angular inertia of the small cone
            // from the angular inertia of the small cone.

            // Create the large and small cone.
            let cone_height =
                max_radius * (self.height / ops::abs(self.radius_top - self.radius_bottom));
            let large_cone = Cone {
                radius: max_radius,
                height: cone_height,
            };
            let small_cone = Cone {
                radius: min_radius,
                height: cone_height - self.height,
            };

            // Compute the volumes of the large and small cone and the frustum.
            // These are equivalent to the masses when using a uniform density of `1.0`.
            let large_cone_volume = large_cone.volume();
            let small_cone_volume = small_cone.volume();
            let volume = large_cone_volume - small_cone_volume;

            // The total mass of the frustum is `1.0` in this case, so we just want the fractional volumes
            // for determining how much each cone contributes to the angular inertia.
            let large_cone_volume_fraction = large_cone_volume / volume;
            let small_cone_volume_fraction = small_cone_volume / volume;

            let large_cone_angular_inertia =
                large_cone.principal_angular_inertia(large_cone_volume_fraction);
            let mut small_cone_angular_inertia =
                small_cone.principal_angular_inertia(small_cone_volume_fraction);

            // The small cone's center of mass is offset from the frustum's center of mass,
            // so we need to take the parallel axis theorem (also known as Steiner's theorem) into account.
            //
            // I = I_cm + m * d^2
            //
            // - `I_cm` is the angular inertia about the center of mass
            // - `m` is the mass, `small_cone_fraction` in this case
            // - `d` is the distance between the parallel axes
            let d = 0.5 * (self.height + small_cone.height) + small_cone.center_of_mass().y;
            let extra_angular_inertia = small_cone_volume_fraction * d * d;
            small_cone_angular_inertia.x += extra_angular_inertia;
            small_cone_angular_inertia.z += extra_angular_inertia;

            // Return the total principal angular inertia.
            large_cone_angular_inertia.x - small_cone_angular_inertia
        }
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        if self.radius_top == self.radius_bottom {
            Vec3::ZERO
        } else {
            // Adapted from: https://mathworld.wolfram.com/ConicalFrustum.html

            let (min_radius, max_radius) = if self.radius_top < self.radius_bottom {
                (self.radius_top, self.radius_bottom)
            } else {
                (self.radius_bottom, self.radius_top)
            };

            let min_radius_squared = min_radius.squared();
            let max_radius_squared = max_radius.squared();
            let radii_product = self.radius_top * self.radius_bottom;

            let y = self.height
                * ((max_radius_squared + 2.0 * radii_product + 3.0 * min_radius_squared)
                    / (4.0 * (max_radius_squared + radii_product + min_radius_squared))
                    - 0.5);

            Vec3::new(0.0, y, 0.0)
        }
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties3d {
        if self.radius_top == self.radius_bottom {
            Cylinder::new(self.radius_top, self.height).mass_properties(density)
        } else {
            // This is a combination of all the methods above, sharing variables where possible.

            let (min_radius, max_radius) = if self.radius_top < self.radius_bottom {
                (self.radius_top, self.radius_bottom)
            } else {
                (self.radius_bottom, self.radius_top)
            };

            // The conical frustum can be thought of as a cone with a smaller cone subtracted from it.
            // To get its angular inertia, we can subtract the angular inertia of the small cone
            // from the angular inertia of the small cone.

            // Create the large and small cone.
            let cone_height =
                max_radius * (self.height / ops::abs(self.radius_top - self.radius_bottom));
            let large_cone = Cone {
                radius: max_radius,
                height: cone_height,
            };
            let small_cone = Cone {
                radius: min_radius,
                height: cone_height - self.height,
            };

            // Compute the volumes of the large and small cone and the frustum.
            // These are equivalent to the masses when using a uniform density of `1.0`.
            let large_cone_volume = large_cone.volume();
            let small_cone_volume = small_cone.volume();
            let volume = large_cone_volume - small_cone_volume;

            // Compute the mass.
            let mass = volume * density;

            // Compute how much each cone contributes to the angular inertia.
            let large_cone_volume_fraction = large_cone_volume / volume;
            let small_cone_volume_fraction = small_cone_volume / volume;

            let large_cone_angular_inertia =
                large_cone.principal_angular_inertia(large_cone_volume_fraction);
            let mut small_cone_angular_inertia =
                small_cone.principal_angular_inertia(small_cone_volume_fraction);

            // The small cone's center of mass is offset from the frustum's center of mass,
            // so we need to take the parallel axis theorem (also known as Steiner's theorem) into account.
            //
            // I = I_cm + m * d^2
            //
            // - `I_cm` is the angular inertia about the center of mass
            // - `m` is the mass, `small_cone_fraction` in this case
            // - `d` is the distance between the parallel axes
            let d = 0.5 * (self.height + small_cone.height) + small_cone.center_of_mass().y;
            let extra_angular_inertia = small_cone_volume_fraction * d * d;
            small_cone_angular_inertia.x += extra_angular_inertia;
            small_cone_angular_inertia.z += extra_angular_inertia;

            // Return the total principal angular inertia.
            let principal_angular_inertia =
                mass * (large_cone_angular_inertia.x - small_cone_angular_inertia);

            let min_radius_squared = min_radius.squared();
            let max_radius_squared = max_radius.squared();
            let radii_product = self.radius_top * self.radius_bottom;

            // Compute the center of mass.
            let y = self.height
                * ((max_radius_squared + 2.0 * radii_product + 3.0 * min_radius_squared)
                    / (4.0 * (max_radius_squared + radii_product + min_radius_squared))
                    - 0.5);
            let center_of_mass = Vec3::new(0.0, y, 0.0);

            MassProperties3d::new(mass, principal_angular_inertia, center_of_mass)
        }
    }
}

impl ComputeMassProperties3d for Torus {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        // Reference: https://en.wikipedia.org/wiki/List_of_moments_of_inertia

        let major_radius_squared = self.major_radius.squared();
        let minor_radius_squared = self.minor_radius.squared();

        let principal = 0.25 * (4.0 * major_radius_squared + 3.0 * minor_radius_squared);
        let off_principal = 1.0 / 8.0 * (4.0 * major_radius_squared + 5.0 * minor_radius_squared);
        Vec3::new(off_principal, principal, off_principal)
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::new(self.unit_principal_angular_inertia())
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl ComputeMassProperties3d for Tetrahedron {
    #[inline]
    fn mass(&self, density: f32) -> f32 {
        self.volume() * density
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        let tensor = self.unit_angular_inertia_tensor();
        tensor.principal_angular_inertia_with_local_frame().0
    }

    #[inline]
    fn local_inertial_frame(&self) -> Quat {
        let tensor = self.unit_angular_inertia_tensor();
        tensor.principal_angular_inertia_with_local_frame().1
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        // References:
        // - F. Tonon. "Explicit Exact Formulas for the 3-D Tetrahedron Inertia Tensor in Terms of its Vertex Coordinates"
        // - Parry: https://github.com/dimforge/parry/blob/837291f1a051dc04e6e4ab8bc2e5b438ce66d257/src/mass_properties/mass_properties_trimesh3d.rs#L38

        let [p1, p2, p3, p4] = self.vertices;

        // For readability
        let x1 = p1.x;
        let y1 = p1.y;
        let z1 = p1.z;
        let x2 = p2.x;
        let y2 = p2.y;
        let z2 = p2.z;
        let x3 = p3.x;
        let y3 = p3.y;
        let z3 = p3.z;
        let x4 = p4.x;
        let y4 = p4.y;
        let z4 = p4.z;

        let diag_x = x1 * x1
            + x1 * x2
            + x2 * x2
            + x1 * x3
            + x2 * x3
            + x3 * x3
            + x1 * x4
            + x2 * x4
            + x3 * x4
            + x4 * x4;
        let diag_y = y1 * y1
            + y1 * y2
            + y2 * y2
            + y1 * y3
            + y2 * y3
            + y3 * y3
            + y1 * y4
            + y2 * y4
            + y3 * y4
            + y4 * y4;
        let diag_z = z1 * z1
            + z1 * z2
            + z2 * z2
            + z1 * z3
            + z2 * z3
            + z3 * z3
            + z1 * z4
            + z2 * z4
            + z3 * z4
            + z4 * z4;

        let a0 = (diag_y + diag_z) * 0.1;
        let b0 = (diag_z + diag_x) * 0.1;
        let c0 = (diag_x + diag_y) * 0.1;

        let a1 = (y1 * z1 * 2.0
            + y2 * z1
            + y3 * z1
            + y4 * z1
            + y1 * z2
            + y2 * z2 * 2.0
            + y3 * z2
            + y4 * z2
            + y1 * z3
            + y2 * z3
            + y3 * z3 * 2.0
            + y4 * z3
            + y1 * z4
            + y2 * z4
            + y3 * z4
            + y4 * z4 * 2.0)
            * 0.05;
        let b1 = (x1 * z1 * 2.0
            + x2 * z1
            + x3 * z1
            + x4 * z1
            + x1 * z2
            + x2 * z2 * 2.0
            + x3 * z2
            + x4 * z2
            + x1 * z3
            + x2 * z3
            + x3 * z3 * 2.0
            + x4 * z3
            + x1 * z4
            + x2 * z4
            + x3 * z4
            + x4 * z4 * 2.0)
            * 0.05;
        let c1 = (x1 * y1 * 2.0
            + x2 * y1
            + x3 * y1
            + x4 * y1
            + x1 * y2
            + x2 * y2 * 2.0
            + x3 * y2
            + x4 * y2
            + x1 * y3
            + x2 * y3
            + x3 * y3 * 2.0
            + x4 * y3
            + x1 * y4
            + x2 * y4
            + x3 * y4
            + x4 * y4 * 2.0)
            * 0.05;

        // TODO: Are -b1 and -c1 flipped here?
        AngularInertiaTensor::from_symmetric_mat3(SymmetricMat3::new(a0, -b1, -c1, b0, -a1, c0))
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        (self.vertices[0] + self.vertices[1] + self.vertices[2] + self.vertices[3]) / 4.0
    }

    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties3d {
        let volume = self.volume();
        let center_of_mass = self.center_of_mass();

        if volume < f32::EPSILON {
            return MassProperties3d::new(0.0, Vec3::ZERO, center_of_mass);
        }

        let mass = volume * density;
        let tensor = self.angular_inertia_tensor(mass);

        MassProperties3d::new_with_angular_inertia_tensor(mass, tensor, center_of_mass)
    }
}

macro_rules! impl_zero_mass_properties_3d {
    ($($shape:ty),*) => {
        $(
            impl ComputeMassProperties3d for $shape {
                #[inline]
                fn mass(&self, _density: f32) -> f32 {
                    0.0
                }

                #[inline]
                fn unit_principal_angular_inertia(&self) -> Vec3 {
                    Vec3::ZERO
                }

                #[inline]
                fn principal_angular_inertia(&self, _mass: f32) -> Vec3 {
                    Vec3::ZERO
                }

                #[inline]
                fn local_inertial_frame(&self) -> Quat {
                    Quat::IDENTITY
                }

                #[inline]
                fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
                    AngularInertiaTensor::ZERO
                }

                #[inline]
                fn angular_inertia_tensor(&self, _mass: f32) -> AngularInertiaTensor {
                    AngularInertiaTensor::ZERO
                }

                #[inline]
                fn center_of_mass(&self) -> Vec3 {
                    Vec3::ZERO
                }

                #[inline]
                fn mass_properties(&self, _density: f32) -> MassProperties3d {
                    MassProperties3d::ZERO
                }
            }
        )*
    };
}

impl_zero_mass_properties_3d!(Plane3d);
impl_zero_mass_properties_3d!(Line3d);
impl_zero_mass_properties_3d!(Segment3d);
impl_zero_mass_properties_3d!(Triangle3d);

impl ComputeMassProperties3d for Polyline3d {
    #[inline]
    fn mass(&self, _density: f32) -> f32 {
        0.0
    }

    #[inline]
    fn unit_principal_angular_inertia(&self) -> Vec3 {
        Vec3::ZERO
    }

    #[inline]
    fn principal_angular_inertia(&self, _mass: f32) -> Vec3 {
        Vec3::ZERO
    }

    #[inline]
    fn local_inertial_frame(&self) -> bevy_math::Quat {
        Quat::IDENTITY
    }

    #[inline]
    fn unit_angular_inertia_tensor(&self) -> AngularInertiaTensor {
        AngularInertiaTensor::ZERO
    }

    #[inline]
    fn angular_inertia_tensor(&self, _mass: f32) -> AngularInertiaTensor {
        AngularInertiaTensor::ZERO
    }

    #[inline]
    fn center_of_mass(&self) -> Vec3 {
        Vec3::ZERO
    }

    #[inline]
    fn mass_properties(&self, _density: f32) -> MassProperties3d {
        MassProperties3d::ZERO
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use approx::assert_relative_eq;
    use bevy_math::{
        bounding::{Bounded3d, BoundingVolume},
        Isometry3d, ShapeSample, Vec3Swizzles,
    };
    use rand::SeedableRng;

    use super::*;

    macro_rules! test_shape {
        ($test_name:tt, $shape:expr, $point_generator:expr) => {
            #[test]
            fn $test_name() {
                let shape = $shape;

                // Sample enough points to have a close enough point cloud representation of the shape.
                let points: Vec<Vec3> = $point_generator(&shape);

                // Compute the mass properties to test.
                let density = 2.0;
                let mass = shape.mass(density);
                let principal_angular_inertia = shape.principal_angular_inertia(mass);
                let local_inertial_frame = shape.local_inertial_frame();
                let angular_inertia_tensor = shape.angular_inertia_tensor(mass);
                let center_of_mass = shape.center_of_mass();

                // First, test that the individually computed properties match the full properties.
                let mass_props = shape.mass_properties(density);
                assert_relative_eq!(mass, mass_props.mass);
                assert_relative_eq!(
                    principal_angular_inertia,
                    mass_props.principal_angular_inertia
                );
                assert_relative_eq!(center_of_mass, mass_props.center_of_mass);

                // Estimate the expected mass properties using the point cloud.
                // Note: We could also approximate the mass using Monte Carlo integration.
                //       This would require point containment checks.
                let expected =
                    MassProperties3d::from_point_cloud(&points, mass, local_inertial_frame);

                assert_relative_eq!(mass, expected.mass);
                assert_relative_eq!(
                    principal_angular_inertia,
                    expected.principal_angular_inertia,
                    epsilon = 0.1
                );
                assert_relative_eq!(center_of_mass, expected.center_of_mass, epsilon = 0.01);

                // Check that the computed tensor matches the tensor computed from
                // the principal moments of inertia and the local inertial frame.
                assert_relative_eq!(
                    angular_inertia_tensor,
                    AngularInertiaTensor::new_with_local_frame(
                        principal_angular_inertia,
                        local_inertial_frame
                    ),
                    epsilon = 1e-5
                );

                // Check that the principal moments of inertia and the local inertial frame
                // extracted from the tensor produce the original tensor. This tests that
                // the diagonalization is correct.
                // Note: The principal moments of inertia are *not* unique, and can differ
                //       from the original values while still producing the same tensor
                //       when taking the local inertial frame into account.
                let (principal, frame) =
                    angular_inertia_tensor.principal_angular_inertia_with_local_frame();
                assert_relative_eq!(
                    AngularInertiaTensor::new_with_local_frame(principal, frame),
                    angular_inertia_tensor,
                    epsilon = 1e-5
                );
            }
        };
    }

    fn sample_shape<S: ShapeSample<Output = Vec3>>(shape: &S) -> Vec<S::Output> {
        let mut rng = rand_chacha::ChaCha8Rng::from_seed(Default::default());
        (0..2_000_000)
            .map(|_| shape.sample_interior(&mut rng))
            .collect::<Vec<_>>()
    }

    fn rejection_sample_shape<S: Bounded3d>(
        func: impl Fn(&S, Vec3) -> bool,
        shape: &S,
    ) -> Vec<Vec3> {
        let mut rng = rand_chacha::ChaCha8Rng::from_seed(Default::default());
        let mut points = Vec::new();
        let aabb = shape.aabb_3d(Isometry3d::IDENTITY);
        let aabb_center: Vec3 = aabb.center().into();
        let cuboid = Cuboid {
            half_size: aabb.half_size().into(),
        };
        while points.len() < 2_000_000 {
            let point = aabb_center + cuboid.sample_interior(&mut rng);
            if func(shape, point) {
                points.push(point);
            }
        }
        points
    }

    test_shape!(sphere, Sphere::new(2.0), sample_shape);
    test_shape!(cuboid, Cuboid::new(1.0, 2.0, 3.0), sample_shape);
    test_shape!(cylinder, Cylinder::new(1.0, 4.0), sample_shape);
    test_shape!(capsule, Capsule3d::new(1.0, 2.0), sample_shape);
    test_shape!(
        cone,
        Cone {
            radius: 1.0,
            height: 2.0,
        },
        |shape| rejection_sample_shape(cone_contains_point, shape)
    );
    test_shape!(
        conical_frustum,
        ConicalFrustum {
            radius_top: 0.5,
            radius_bottom: 1.0,
            height: 1.0
        },
        |shape| rejection_sample_shape(conical_frustum_contains_point, shape)
    );
    test_shape!(torus, Torus::new(1.0, 2.0), |shape| rejection_sample_shape(
        torus_contains_point,
        shape,
    ));
    test_shape!(
        tetrahedron,
        Tetrahedron::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        sample_shape
    );

    // TODO: This should be removed once Bevy either has this built-in or it has uniform sampling for cones.
    fn cone_contains_point(cone: &Cone, point: Vec3) -> bool {
        let half_height = cone.height * 0.5;

        if point.y < -half_height || point.y > half_height {
            return false;
        }

        // a = tip
        // b = base center
        let pb_dot_ba = -cone.height * (point.y + half_height);

        // Compute the radius of the circular slice.
        // Derived geometrically from the triangular cross-section.
        //
        // let y = pb_dot_ba / cone.height;
        // let slope = cone.height / cone.radius;
        //
        // let delta_radius = y / slope
        //                  = y / (cone.height / cone.radius)
        //                  = y * cone.radius / cone.height
        //                  = pb_dot_ba / cone.height * cone.radius / cone.height
        //                  = pb_dot_ba * cone.radius / (cone.height * cone.height);
        // let radius = cone.radius + delta_radius;

        let delta_radius = pb_dot_ba * cone.radius / cone.height.squared();
        let radius = cone.radius + delta_radius;

        // The squared orthogonal distance from the cone axis
        let ortho_distance_squared = point.xz().length_squared();

        ortho_distance_squared < radius * radius
    }

    // TODO: This should be removed once Bevy either has this built-in or it has uniform sampling for conical frusta.
    fn conical_frustum_contains_point(frustum: &ConicalFrustum, point: Vec3) -> bool {
        let half_height = frustum.height * 0.5;

        if point.y < -half_height || point.y > half_height {
            return false;
        }

        // a = top center
        // b = bottom center
        let pb_dot_ba = -frustum.height * (point.y + half_height);

        // Compute the radius of the circular slice.
        // Derived geometrically from the trapezoidal cross-section.
        //
        // let y = pb_dot_ba / frustum.height;
        // let slope = frustum.height / (frustum.radius_bottom - frustum.radius_top);
        //
        // let delta_radius = y / slope
        //                  = y / (frustum.height / (frustum.radius_bottom - frustum.radius_top))
        //                  = y * (frustum.radius_bottom - frustum.radius_top) / frustum.height
        //                  = pb_dot_ba / frustum.height * (frustum.radius_bottom - frustum.radius_top) / frustum.height
        //                  = pb_dot_ba * (frustum.radius_bottom - frustum.radius_top) / (frustum.height * frustum.height);
        // let radius = frustum.radius_bottom + delta_radius;

        let delta_radius =
            pb_dot_ba * (frustum.radius_bottom - frustum.radius_top) / frustum.height.squared();
        let radius = frustum.radius_bottom + delta_radius;

        // The squared orthogonal distance from the frustum axis
        let ortho_distance_squared = point.xz().length_squared();

        ortho_distance_squared < radius * radius
    }

    // TODO: This should be removed once Bevy either has this built-in or it has uniform sampling for tori.
    fn torus_contains_point(torus: &Torus, point: Vec3) -> bool {
        let minor_radius_squared = torus.minor_radius * torus.minor_radius;
        (torus.major_radius - point.xz().length()).squared() + point.y.squared()
            < minor_radius_squared
    }
}
