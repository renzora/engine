//! `bevy_heavy` is a crate for computing mass properties ([mass], [angular inertia], and [center of mass])
//! for the [geometric primitives] in the [Bevy game engine][Bevy]. This is typically required
//! for things like physics simulations.
//!
//! [mass]: #mass
//! [angular inertia]: #angular-inertia
//! [center of mass]: #center-of-mass
//! [geometric primitives]: bevy_math::primitives
//! [Bevy]: https://bevyengine.org
//!
//! # Usage
//!
//! Mass properties can be computed individually for shapes using the [`mass`], [`angular_inertia`],
//! and [`center_of_mass`] methods:
//!
//! [`mass`]: ComputeMassProperties2d::mass
//! [`angular_inertia`]: ComputeMassProperties2d::angular_inertia
//! [`center_of_mass`]: ComputeMassProperties2d::center_of_mass
//!
//! ```
//! use bevy_heavy::{ComputeMassProperties2d, MassProperties2d};
//! use bevy_math::{primitives::Rectangle, Vec2};
//!
//! let rectangle = Rectangle::new(2.0, 1.0);
//! let density = 2.0;
//!
//! let mass = rectangle.mass(density);
//! let angular_inertia = rectangle.angular_inertia(mass);
//! let center_of_mass = rectangle.center_of_mass();
//! ```
//!
//! You can also compute all mass properties at once, returning [`MassProperties2d`] for 2D shapes,
//! or [`MassProperties3d`] for 3D shapes. This can be more efficient when more than one property is needed.
//!
//! ```
//! # use bevy_heavy::{ComputeMassProperties2d, MassProperties2d};
//! # use bevy_math::{primitives::Rectangle, Vec2};
//! #
//! # let rectangle = Rectangle::new(2.0, 1.0);
//! # let density = 2.0;
//! #
//! let mass_props = rectangle.mass_properties(density);
//! ```
//!
//! The mass property types have several helper methods for various transformations and operations:
//!
//! ```
//! # use bevy_heavy::{ComputeMassProperties2d, MassProperties2d};
//! # use bevy_math::{primitives::Rectangle, Vec2};
//! #
//! # let rectangle = Rectangle::new(2.0, 1.0);
//! # let density = 2.0;
//! #
//! # let mass_props = rectangle.mass_properties(density);
//! #
//! let shifted_inertia = mass_props.shifted_angular_inertia(Vec2::new(-3.5, 1.0));
//! let global_center_of_mass = mass_props.global_center_of_mass(Vec2::new(5.0, 7.5));
//! ```
//!
//! You can also add and subtract mass properties:
//!
//! ```
//! # use bevy_heavy::{ComputeMassProperties2d, MassProperties2d};
//! # use bevy_math::{primitives::Rectangle, Vec2};
//! #
//! # let rectangle = Rectangle::new(2.0, 1.0);
//! # let density = 2.0;
//! #
//! # let mass = rectangle.mass(density);
//! # let angular_inertia = rectangle.angular_inertia(mass);
//! #
//! # let mass_props = rectangle.mass_properties(density);
//! #
//! let mass_props_2 = MassProperties2d::new(mass, angular_inertia, Vec2::new(0.0, 1.0));
//! let sum = mass_props + mass_props_2;
//! approx::assert_relative_eq!(sum - mass_props_2, mass_props);
//! ```
//!
//! To support mass property computation for custom shapes, implement [`ComputeMassProperties2d`]
//! or [`ComputeMassProperties3d`] for them.
//!
//! # Terminology
//!
//! ## Mass
//!
//! **[Mass](https://en.wikipedia.org/wiki/Mass)** is a scalar value representing resistance
//! to linear acceleration when a force is applied.
//!
//! Mass is commonly measured in kilograms (kg).
//!
//! ## Angular Inertia
//!
//! **[Angular inertia](https://en.wikipedia.org/wiki/Moment_of_inertia)**, also known as
//! the **moment of inertia** or **rotational inertia**, is the rotational analog of mass.
//! It represents resistance to angular acceleration when a torque is applied.
//!
//! An object's angular inertia depends on its mass, shape, and how the mass is distributed
//! relative to a rotational axis. It increases with mass and distance from the axis.
//!
//! In 2D, angular inertia can be treated as a scalar value, as it is only defined
//! relative to the Z axis.
//!
//! In 3D, angular inertia can be represented with a [symmetric], [positive-semidefinite] 3x3 [tensor]
//! ([`AngularInertiaTensor`]) that describes the moment of inertia for rotations about the X, Y, and Z axes.
//! By [diagonalizing] this matrix, it is possible to extract the [principal axes of inertia] (a [`Vec3`])
//! and a local inertial frame (a [`Quat`]) that defines the XYZ axes.
//!
//! The latter diagonalized representation is more compact and often easier to work with,
//! but the full tensor can be more efficient for computations using the angular inertia.
//!
//! Angular inertia is commonly measured in kilograms times meters squared (kg⋅m²).
//!
//! [symmetric]: https://en.wikipedia.org/wiki/Symmetric_matrix
//! [positive-semidefinite]: https://en.wikipedia.org/wiki/Definite_matrix
//! [tensor]: https://en.wikipedia.org/wiki/Moment_of_inertia#Inertia_tensor
//! [diagonalizing]: https://en.wikipedia.org/wiki/Diagonalizable_matrix#Diagonalization
//! [principal axes of inertia]: https://en.wikipedia.org/wiki/Moment_of_inertia#Principal_axes
//! [`Vec3`]: bevy_math::Vec3
//! [`Quat`]: bevy_math::Quat
//!
//! ## Center of Mass
//!
//! The **[center of mass](https://en.wikipedia.org/wiki/Center_of_mass)** is the average position
//! of mass in an object. Applying a force at the center of mass causes linear acceleration
//! without angular acceleration.
//!
//! If an object has uniform density, mass is evenly distributed,
//! and the center of mass is at the [geometric center], also known as the [centroid].
//!
//! The center of mass is commonly measured in meters (m).
//!
//! [geometric center]: https://en.wikipedia.org/wiki/Centroid
//! [centroid]: https://en.wikipedia.org/wiki/Centroid

#![warn(missing_docs)]
#![no_std]

#[cfg(any(feature = "2d", feature = "3d"))]
extern crate alloc;

#[cfg(feature = "2d")]
mod dim2;
#[cfg(feature = "3d")]
mod dim3;
mod math_ext;

#[cfg(feature = "2d")]
pub use dim2::*;
#[cfg(feature = "3d")]
pub use dim3::*;
pub use glam_matrix_extras::{Mat3Ext, MatConversionError, SquareMatExt, SymmetricMat3};
pub use math_ext::RecipOrZero;
