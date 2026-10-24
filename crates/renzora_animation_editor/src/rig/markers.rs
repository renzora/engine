//! The points a rig is built from, clicked onto the model on the stage.
//!
//! Press Place Points (or a point's row) and the next click on the model
//! drops that point there, then moves on to the next one still unplaced, so a
//! whole set is one click per point. Esc stops. The click itself is handled by
//! the stage (`stage::tools::place_marker`).
//!
//! Points are in the model's own space. That is what the stage gives for free:
//! its copy of the model sits at the origin, unturned and unscaled, so where a
//! click lands on it is where the point is in the model file. Placing them in
//! the scene viewport used to mean undoing wherever the model had been put in
//! the level first.

use bevy::prelude::*;

/// Which rig a set of markers is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MarkerSet {
    Face,
    Body,
}

impl MarkerSet {
    pub(crate) fn labels(self) -> &'static [&'static str] {
        match self {
            MarkerSet::Face => &["Mouth corner (either side)", "Upper lip, middle", "Lower lip, middle", "Chin"],
            MarkerSet::Body => &["Chin", "Shoulder (either side)", "Elbow", "Wrist", "Groin", "Knee", "Ankle"],
        }
    }

    /// Whether point `i` is one side of a pair, the other side mirrored from
    /// it.
    pub(crate) fn mirrored(self, i: usize) -> bool {
        match self {
            MarkerSet::Face => i == 0,
            MarkerSet::Body => matches!(i, 1 | 2 | 3 | 5 | 6),
        }
    }

    pub(crate) fn colour(self, i: usize) -> Color {
        const PALETTE: [(f32, f32, f32); 7] =
            [(0.56, 0.69, 1.0), (0.94, 0.70, 0.48), (0.88, 0.48, 0.75), (0.50, 0.82, 0.66), (0.77, 0.61, 0.95), (0.95, 0.83, 0.29), (0.44, 0.78, 0.84)];
        let (r, g, b) = PALETTE[i % PALETTE.len()];
        Color::srgb(r, g, b)
    }
}

/// The markers placed so far, and which one the next click places.
#[derive(Resource, Default)]
pub(crate) struct Markers {
    pub(crate) face: Vec<Option<Vec3>>,
    pub(crate) body: Vec<Option<Vec3>>,
    pub(crate) placing: Option<(MarkerSet, usize)>,
    /// The model the markers were placed on; they are cleared when the
    /// panels move to another one.
    pub(crate) root: Option<Entity>,
}

impl Markers {
    pub(crate) fn points(&self, set: MarkerSet) -> &[Option<Vec3>] {
        match set {
            MarkerSet::Face => &self.face,
            MarkerSet::Body => &self.body,
        }
    }

    fn points_mut(&mut self, set: MarkerSet) -> &mut Vec<Option<Vec3>> {
        let v = match set {
            MarkerSet::Face => &mut self.face,
            MarkerSet::Body => &mut self.body,
        };
        v.resize(set.labels().len(), None);
        v
    }

    pub(crate) fn complete(&self, set: MarkerSet) -> Option<Vec<Vec3>> {
        let pts = self.points(set);
        (pts.len() == set.labels().len() && pts.iter().all(Option::is_some)).then(|| pts.iter().flatten().copied().collect())
    }

    pub(crate) fn start(&mut self, set: MarkerSet, index: usize) {
        self.points_mut(set);
        self.placing = Some((set, index));
    }

    /// Place point `index` at `p`, and move on to the next one unplaced.
    pub(crate) fn set(&mut self, set: MarkerSet, index: usize, p: Vec3) {
        let pts = self.points_mut(set);
        pts[index] = Some(p);
        let next = (1..=pts.len()).map(|o| (index + o) % pts.len()).find(|&i| pts[i].is_none());
        self.placing = next.map(|i| (set, i));
    }

    pub(crate) fn clear(&mut self, set: MarkerSet) {
        self.points_mut(set).iter_mut().for_each(|p| *p = None);
        if self.placing.is_some_and(|(s, _)| s == set) {
            self.placing = None;
        }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<Markers>().add_systems(
        Update,
        follow_subject.run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// Markers belong to one model: switching to another clears them.
fn follow_subject(state: Option<Res<crate::AnimationEditorState>>, mut markers: ResMut<Markers>) {
    let subject = state.and_then(|s| s.selected_entity);
    if subject.is_some() && markers.root != subject {
        *markers = Markers { root: subject, ..default() };
    }
}
