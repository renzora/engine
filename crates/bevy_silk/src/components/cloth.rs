use crate::{
    stick::{StickGeneration, StickLen, StickMode},
    vertex_anchor::VertexAnchor,
};
use bevy::{
    ecs::prelude::Component,
    log,
    math::{Mat4, Vec3},
    platform::collections::HashMap,
    prelude::{Entity, GlobalTransform},
};

/// A stick is defined by the two ids of the connectecte points
pub type StickId = [usize; 2];

macro_rules! get_point {
    ($id:expr, $points:expr, $anchored_points:expr) => {
        match $points.get($id) {
            None => {
                log::warn!("Failed to retrieve a Cloth point at index {}", $id);
                continue;
            }
            Some(p) => (*p, $anchored_points.contains_key(&$id)),
        }
    };
}

/// Cloth component. Do not insert it directly, use [`ClothBuilder`] instead.
///
/// [`ClothBuilder`]: crate::prelude::ClothBuilder
#[derive(Debug, Clone, Component, Default)]
#[must_use]
pub struct Cloth {
    /// cloth points unaffected by physics and following an anchor
    /// The key is the point index and the value is a tuple with:
    /// - 0: The [`VertexAnchor`] anchor
    /// - 1: The initial local space vertex position
    pub anchored_points: HashMap<usize, (VertexAnchor, Vec3)>,
    /// Current Cloth points 3D positions in world space
    pub current_point_positions: Vec<Vec3>,
    /// Old Cloth points 3D positions in world space
    pub previous_point_positions: Vec<Vec3>,
    /// Cloth sticks lengths
    ///
    /// * key: array of the two connected points indexes
    /// * value: the target distance between the points
    ///
    /// Note: this field will be automatically populated from mesh data
    pub stick_lengths: HashMap<StickId, f32>,
    /// Cloth sticks behaviour modes
    ///
    /// * key: array of the two connected points indexes
    /// * value: the stick mode
    pub stick_modes: HashMap<StickId, StickMode>,
}

impl Cloth {
    /// Computes the new local vertex positions of the cloth mesh
    ///
    /// # Arguments
    ///
    /// * `transform` - the `GlobalTransform` associated to the cloth entity
    #[must_use]
    pub fn compute_vertex_positions(
        &self,
        transform: &GlobalTransform,
    ) -> impl ExactSizeIterator<Item = Vec3> + '_ {
        let matrix = transform.to_matrix().inverse();

        // World space positions..
        self.current_point_positions
            .iter()
            // ..computed to local space
            .map(move |p| matrix.transform_point3(*p))
    }

    /// Creates a new cloth from a mesh. Points positions will be directly
    /// extracted from the given vertex positions and the sticks will be
    /// extracted from the given `indices` (triangles) according to
    /// the associated [`StickGeneration`] and [`StickLen`].
    ///
    /// # Arguments
    ///
    /// * `vertex_positions` - the mesh vertex positions
    /// * `indices` - the mesh indices
    /// * `anchored_points` - the pinned vertex position indices
    /// * `stick_generation` - The stick generation mode
    /// * `stick_len` - The stick length option
    /// * `transform_matrix` - the transform matrix of the associated
    ///   `GlobalTransform`
    ///
    /// # Panics
    ///
    /// May panic if `anchored_points` contains an out of bounds vertex id.
    pub fn new(
        vertex_positions: &[Vec3],
        indices: &[u32],
        anchored_points: HashMap<usize, VertexAnchor>,
        stick_generation: StickGeneration,
        stick_len: StickLen,
        stick_mode: StickMode,
        transform_matrix: &Mat4,
    ) -> Self {
        let anchored_points = anchored_points
            .into_iter()
            .map(|(i, anchor)| {
                let pos = vertex_positions
                    .get(i)
                    .unwrap_or_else(|| panic!("Anchored vertex id {i} is out of bounds"));
                (i, (anchor, *pos))
            })
            .collect();
        let positions: Vec<Vec3> = vertex_positions
            .iter()
            .map(|p| transform_matrix.transform_point3(*p))
            .collect();
        let indices: Vec<usize> = indices.iter().map(|i| *i as usize).collect();
        if !indices.len().is_multiple_of(3) {
            log::error!("Mesh indices count is not a multiple of 3, some indices will be skipped",);
        }
        let mut stick_lengths = HashMap::with_capacity(indices.len() / 3);
        for truple in indices.chunks_exact(3) {
            let [a, b, c] = [truple[0], truple[1], truple[2]];
            let [p_a, p_b, p_c] = [positions[a], positions[b], positions[c]];
            if !stick_lengths.contains_key(&[b, a]) {
                stick_lengths.insert([a, b], stick_len.get_len(p_a, p_b));
            }
            if !stick_lengths.contains_key(&[c, b]) {
                stick_lengths.insert([b, c], stick_len.get_len(p_b, p_c));
            }
            if stick_generation == StickGeneration::Triangles
                && !stick_lengths.contains_key(&[a, c])
            {
                stick_lengths.insert([c, a], stick_len.get_len(p_c, p_a));
            }
        }
        let stick_modes = stick_lengths.keys().map(|id| (*id, stick_mode)).collect();
        Self {
            anchored_points,
            current_point_positions: positions.clone(),
            previous_point_positions: positions,
            stick_lengths,
            stick_modes,
        }
    }

    /// Changes the stick behaviour to `new_mode` for `sticks`
    pub fn edit_stick_modes(&mut self, sticks: &[StickId], new_mode: StickMode) {
        log::debug!("Editing {} sticks: {new_mode:#?}", sticks.len());
        for id in sticks {
            self.stick_modes.get_mut(id).map_or_else(
                || {
                    log::warn!("Attempted to edit missing stick `{id:?}` behaviour");
                },
                |mode| {
                    *mode = new_mode;
                },
            );
        }
    }

    /// Adds an extra point to the cloth (Not included in the base mesh) and
    /// returns its id and associated stick ids.
    pub fn add_point(
        &mut self,
        pos: Vec3,
        stick_mode: StickMode,
        anchor: Option<VertexAnchor>,
        transform_matrix: &Mat4,
        connects_to: impl Fn(usize, &Vec3) -> bool,
    ) -> (usize, Vec<StickId>) {
        let center = transform_matrix.transform_point3(pos);
        self.current_point_positions.push(center);
        self.previous_point_positions.push(center);
        let id = self.current_point_positions.len().saturating_sub(1);
        let sticks: Vec<_> = self
            .current_point_positions
            .iter()
            .enumerate()
            .filter(|(i, p)| connects_to(*i, p))
            .map(|(i, p)| {
                let stick_id = [id, i];
                self.stick_modes.insert(stick_id, stick_mode);
                self.stick_lengths.insert(stick_id, p.distance(center));
                stick_id
            })
            .collect();
        log::debug!(
            "Added custom point {pos:?} and {} sticks with to it: {stick_mode:#?}",
            sticks.len()
        );
        if let Some(anchor) = anchor {
            self.anchored_points.insert(id, (anchor, pos));
        }
        (id, sticks)
    }

    /// Solves cloth points collisions, moving them outside of colliders
    ///
    /// # Arguments
    ///
    /// * `solve_point` - function taking a cloth point and returning the new
    ///   solved point
    pub fn solve_collisions(&mut self, solve_point: impl Fn(&Vec3) -> Option<Vec3>) {
        for (point, new_point) in self
            .current_point_positions
            .iter_mut()
            .enumerate()
            .filter(|(i, _p)| !self.anchored_points.contains_key(i))
            .filter_map(|(_i, p)| solve_point(p).map(|np| (p, np)))
        {
            *point = new_point;
        }
    }

    /// Updates the cloth anchored points
    ///
    /// # Arguments
    ///
    /// * `transform` - The `GlobalTransform` associated to the cloth entity
    /// * `anchor_query` - A function allowing to retrieve the `GlobalTransform`
    ///   of a given entity
    pub fn update_anchored_points<'a>(
        &mut self,
        transform: &GlobalTransform,
        anchor_query: impl Fn(Entity) -> Option<&'a GlobalTransform>,
    ) {
        for (i, (anchor, inital_pos)) in &self.anchored_points {
            self.current_point_positions[*i] =
                anchor.get_position(*inital_pos, transform, &anchor_query);
        }
    }

    /// Updates the cloth points according to their own velocity and external
    /// friction and acceleration
    ///
    /// # Arguments
    ///
    /// * `friction` - Friction to apply to the points velocity
    /// * `acceleration` - Global acceleration force (gravity, wind, etc)
    pub fn update_points(&mut self, friction: f32, acceleration: Vec3) {
        let position_cache = self.current_point_positions.clone();
        for (i, point) in self.current_point_positions.iter_mut().enumerate() {
            if !self.anchored_points.contains_key(&i) {
                let velocity = self
                    .previous_point_positions
                    .get(i)
                    .map_or(Vec3::ZERO, |prev| *point - *prev);
                *point += velocity * friction + acceleration * friction;
            }
        }
        self.previous_point_positions = position_cache;
    }

    /// Applies the cloth sticks constraints
    ///
    /// # Arguments
    ///
    /// * `depth` - Number of sticks constraint iterations
    pub fn update_sticks(&mut self, depth: u8) {
        for _ in 0..depth {
            for ([id_a, id_b], target_len) in &self.stick_lengths {
                let (position_a, fixed_a) =
                    get_point!(*id_a, self.current_point_positions, self.anchored_points);
                let (position_b, fixed_b) =
                    get_point!(*id_b, self.current_point_positions, self.anchored_points);
                if fixed_a && fixed_b {
                    continue;
                }
                let target_len = match self.stick_modes[&[*id_a, *id_b]] {
                    StickMode::Fixed => *target_len,
                    StickMode::Spring {
                        min_percent,
                        max_percent,
                    } => {
                        let dist = position_a.distance(position_b) / *target_len;
                        if dist < min_percent {
                            *target_len * min_percent
                        } else if dist > max_percent {
                            *target_len * max_percent
                        } else {
                            continue;
                        }
                    }
                };
                let center = (position_b + position_a) / 2.0;
                let direction = match (position_b - position_a).try_normalize() {
                    None => {
                        log::warn!(
                            "Failed handle stick between points {} and {} which are too close to \
                             each other",
                            *id_a,
                            *id_b
                        );
                        continue;
                    }
                    Some(dir) => dir * target_len / 2.0,
                };
                if !fixed_a {
                    self.current_point_positions[*id_a] = if fixed_b {
                        position_b - direction * 2.0
                    } else {
                        center - direction
                    };
                }
                if !fixed_b {
                    self.current_point_positions[*id_b] = if fixed_a {
                        position_a + direction * 2.0
                    } else {
                        center + direction
                    };
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::mesh::rectangle_mesh;

    mod init_from_mesh {
        use super::*;
        use crate::components::cloth_rendering::ClothRendering;
        use bevy::transform::prelude::Transform;

        fn expected_stick_len(
            len: usize,
            generation: StickGeneration,
            (size_x, size_y): (usize, usize),
        ) {
            match generation {
                StickGeneration::Quads => {
                    assert_eq!(len, (size_x - 1) * size_y + (size_y - 1) * size_x);
                }
                StickGeneration::Triangles => {
                    assert_eq!(
                        len,
                        (size_x - 1) * size_y + (size_y - 1) * size_x + (size_x - 1) * (size_y - 1)
                    );
                }
            }
        }

        #[test]
        fn works_with_quads() {
            let mesh = rectangle_mesh((100, 100), (Vec3::X, -Vec3::Y), Vec3::Z);
            let matrix = Transform::default().to_matrix();
            let cloth_rendering = ClothRendering::init(&mesh, Default::default()).unwrap();
            let cloth = Cloth::new(
                &cloth_rendering.vertex_positions,
                &cloth_rendering.indices,
                Default::default(),
                StickGeneration::Quads,
                StickLen::Auto,
                StickMode::Fixed,
                &matrix,
            );
            assert_eq!(cloth.current_point_positions.len(), 100 * 100);
            assert_eq!(cloth.previous_point_positions.len(), 100 * 100);
            expected_stick_len(
                cloth.stick_lengths.len(),
                StickGeneration::Quads,
                (100, 100),
            );
        }

        #[test]
        fn works_with_quads_2() {
            let mesh = rectangle_mesh((66, 42), (Vec3::X, -Vec3::Y), Vec3::Z);
            let matrix = Transform::default().to_matrix();
            let cloth_rendering = ClothRendering::init(&mesh, Default::default()).unwrap();
            let cloth = Cloth::new(
                &cloth_rendering.vertex_positions,
                &cloth_rendering.indices,
                Default::default(),
                StickGeneration::Quads,
                StickLen::Auto,
                StickMode::Fixed,
                &matrix,
            );
            assert_eq!(cloth.current_point_positions.len(), 66 * 42);
            assert_eq!(cloth.previous_point_positions.len(), 66 * 42);
            expected_stick_len(cloth.stick_lengths.len(), StickGeneration::Quads, (66, 42));
        }

        #[test]
        fn works_with_triangles() {
            let mesh = rectangle_mesh((100, 100), (Vec3::X, -Vec3::Y), Vec3::Z);
            let matrix = Transform::default().to_matrix();
            let cloth_rendering = ClothRendering::init(&mesh, Default::default()).unwrap();
            let cloth = Cloth::new(
                &cloth_rendering.vertex_positions,
                &cloth_rendering.indices,
                Default::default(),
                StickGeneration::Triangles,
                StickLen::Auto,
                StickMode::Fixed,
                &matrix,
            );
            assert_eq!(cloth.current_point_positions.len(), 100 * 100);
            assert_eq!(cloth.previous_point_positions.len(), 100 * 100);
            expected_stick_len(
                cloth.stick_lengths.len(),
                StickGeneration::Triangles,
                (100, 100),
            );
        }

        #[test]
        fn works_with_triangles_2() {
            let mesh = rectangle_mesh((66, 42), (Vec3::X, -Vec3::Y), Vec3::Z);
            let matrix = Transform::default().to_matrix();
            let cloth_rendering = ClothRendering::init(&mesh, Default::default()).unwrap();
            let cloth = Cloth::new(
                &cloth_rendering.vertex_positions,
                &cloth_rendering.indices,
                Default::default(),
                StickGeneration::Triangles,
                StickLen::Auto,
                StickMode::Fixed,
                &matrix,
            );
            assert_eq!(cloth.current_point_positions.len(), 66 * 42);
            assert_eq!(cloth.previous_point_positions.len(), 66 * 42);
            expected_stick_len(
                cloth.stick_lengths.len(),
                StickGeneration::Triangles,
                (66, 42),
            );
        }
    }
}
