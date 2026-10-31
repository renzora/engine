//! Streaming: which parts of each converted mesh are on the GPU.
//!
//! A mesh's clusters are loaded and evicted a DAG group at a time (see
//! `renzora_rvg::GroupGraph` for the rule that decides what is drawn from a
//! partly resident mesh, and why residency must stay closed upward). Each
//! loaded group is a self-contained page in the pool: the vertices its
//! clusters use, their vertex lists as offsets into those, and their triangle
//! bytes. Every cluster's record stays on the GPU whether its group is
//! resident or not (it is small, and the cull pass needs its bounds to know
//! what to ask for); a resident group's records point into its page.
//!
//! The loop, each frame:
//!
//! 1. The cull pass, per view, marks two bits per group: *requested*, a group
//!    whose finer clusters are wanted but missing, and *drawn*.
//! 2. After the frame is submitted ([`feedback`]), both bitsets are copied to
//!    a staging buffer, cleared for the next frame, and mapped. They arrive a
//!    frame or two later.
//! 3. [`Streaming::update`] loads requested groups (coarse ones first, each
//!    after the groups above it) within an upload budget per frame, making
//!    room within the memory budget by evicting the groups drawn least
//!    recently that nothing resident depends on.
//!
//! Roots are pinned: they are the stand-in of last resort. Skinned meshes are
//! pinned whole, as one page: the skin pass deforms a mesh's vertices as one
//! contiguous run, and characters are small next to the scenes this exists for.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_resource::{
    Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, MapMode,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use renzora_rvg::{Attributes, GroupGraph, RvgMesh};

use super::pool::{Page, Pool};
use super::scene::{pack_attributes, sphere, GpuCluster, MeshKey, NO_GROUP};

/// The most page data uploaded in one frame. A camera cut can ask for
/// hundreds of megabytes at once; spreading them keeps the frame from
/// stalling, and the coarser stand-ins cover the gap meanwhile.
const UPLOAD_BUDGET: u64 = 16 << 20;

/// A group drawn within this many frames is never evicted, so a page is not
/// thrown out and asked for again while the feedback is still in flight.
const KEEP_FRAMES: u32 = 8;

/// Staging buffers in flight: the feedback of one frame is read two or three
/// frames later.
const FEEDBACK_SLOTS: usize = 3;

enum Residency {
    /// Pinned as one page: skinned meshes.
    Whole(Page),
    /// Streamed group by group.
    Grouped {
        graph: GroupGraph,
        pages: Vec<Option<Page>>,
    },
}

struct StreamedMesh {
    mesh: Arc<RvgMesh>,
    residency: Residency,
    /// Its clusters' records, with page offsets filled in for resident
    /// groups and group numbers still local to the mesh.
    records: Vec<GpuCluster>,
    first_cluster: u32,
    group_base: u32,
}

/// One page's data, built on the CPU before it is uploaded.
struct PageData {
    positions: Vec<f32>,
    attributes: Vec<u32>,
    words: Vec<u32>,
    /// Per cluster of the page: its vertex list's offset and its triangles'
    /// byte offset, both within the page's words.
    offsets: Vec<(u32, u32)>,
}

impl PageData {
    fn bytes(&self) -> u64 {
        (self.positions.len() * 4 + self.attributes.len() * 4 + self.words.len() * 4) as u64
    }
}

/// The clusters `clusters` of `mesh` as one page: only the vertices they use,
/// renumbered from zero.
fn build_page(mesh: &RvgMesh, clusters: std::ops::Range<usize>) -> PageData {
    let mut local: HashMap<u32, u32> = HashMap::new();
    let mut order: Vec<u32> = Vec::new();
    let mut lists: Vec<u32> = Vec::new();
    let mut triangles: Vec<u8> = Vec::new();
    let mut offsets = Vec::with_capacity(clusters.len());
    for c in &mesh.clusters[clusters] {
        offsets.push((lists.len() as u32, triangles.len() as u32));
        let vertices = &mesh.cluster_vertices[c.vertex_offset as usize..][..c.vertex_count as usize];
        lists.extend(vertices.iter().map(|&v| {
            *local.entry(v).or_insert_with(|| {
                order.push(v);
                order.len() as u32 - 1
            })
        }));
        let start = c.triangle_offset as usize * 3;
        triangles.extend_from_slice(&mesh.cluster_triangles[start..start + c.triangle_count as usize * 3]);
    }
    // Triangle bytes start on a word, after the lists.
    let triangle_base = lists.len() as u32 * 4;
    for o in &mut offsets {
        o.1 += triangle_base;
    }
    let mut words = lists;
    words.extend(triangles.chunks(4).map(|w| {
        let mut word = [0u8; 4];
        word[..w.len()].copy_from_slice(w);
        u32::from_le_bytes(word)
    }));

    let mut sub = RvgMesh {
        attributes: mesh.attributes,
        ..Default::default()
    };
    let pick = |v: &Vec<[f32; 3]>| order.iter().map(|&i| v[i as usize]).collect::<Vec<_>>();
    sub.positions = pick(&mesh.positions);
    if mesh.attributes.has(Attributes::NORMAL) {
        sub.normals = pick(&mesh.normals);
    }
    if mesh.attributes.has(Attributes::UV0) {
        sub.uv0 = order.iter().map(|&i| mesh.uv0[i as usize]).collect();
    }
    if mesh.attributes.has(Attributes::TANGENT) {
        sub.tangents = order.iter().map(|&i| mesh.tangents[i as usize]).collect();
    }
    let mut attributes = Vec::with_capacity(order.len() * 4);
    pack_attributes(&sub, &mut attributes);
    PageData {
        positions: sub.positions.into_flattened(),
        attributes,
        words,
        offsets,
    }
}

/// The record of a cluster whose group is not resident: bounds only.
fn record(mesh: &RvgMesh, index: usize) -> GpuCluster {
    let c = &mesh.clusters[index];
    GpuCluster {
        culling: sphere(&c.culling),
        lod: sphere(&c.lod),
        parent: sphere(&c.parent),
        lod_error: c.lod.error,
        parent_error: c.parent.error.min(3.4e38),
        vertex_offset: 0,
        triangle_offset: 0,
        triangle_count: c.triangle_count,
        level: mesh.groups[c.group as usize].depth,
        group: c.group,
        refined: if c.refined < 0 { NO_GROUP } else { c.refined as u32 },
        vertex_base: 0,
        _pad: [0; 3],
    }
}

/// Where a mesh's whole vertex range sits, for the instances that draw it.
pub(crate) struct MeshPlacement {
    pub first_cluster: u32,
    pub cluster_count: u32,
    /// The first vertex of a mesh pinned whole (skinned); 0 for a streamed one.
    pub vertex_base: u32,
}

/// Feedback in flight: one staging buffer per slot, and the words it read back
/// once mapped.
struct FeedbackSlot {
    staging: Buffer,
    busy: bool,
    result: Arc<Mutex<Option<Vec<u32>>>>,
}

#[derive(Default)]
pub(crate) struct Streaming {
    pool: Option<Pool>,
    meshes: Vec<(MeshKey, StreamedMesh)>,
    /// Per group, numbered across the scene.
    owner: Vec<(u32, u32)>,
    resident: Vec<bool>,
    pinned: Vec<bool>,
    resident_children: Vec<u32>,
    last_drawn: Vec<u32>,
    depth: Vec<u32>,
    /// What the GPU has for every cluster, kept in step with `records`.
    pub clusters: Option<Buffer>,
    /// One bit per group: resident.
    pub residency: Option<Buffer>,
    /// One bit per group each, written by the cull pass.
    pub requests: Option<Buffer>,
    pub drawn: Option<Buffer>,
    words: u32,
    dirty_clusters: Vec<(u32, u32)>,
    residency_dirty: bool,
    feedback: Vec<FeedbackSlot>,
    next_slot: usize,
    frame: u32,
    /// Bumped whenever a buffer is replaced, so bind groups follow.
    pub generation: u64,
    /// Reporting: last values logged.
    reported: (u64, u64),
    /// Groups evicted since the scene's meshes last changed.
    pub evicted: u64,
}

impl Streaming {
    pub fn ready(&self) -> bool {
        self.pool.is_some() && self.clusters.is_some() && self.residency.is_some()
    }

    pub fn pool(&self) -> Option<&Pool> {
        self.pool.as_ref()
    }

    pub fn placement(&self, key: &MeshKey) -> Option<MeshPlacement> {
        let (_, m) = self.meshes.iter().find(|(k, _)| k == key)?;
        Some(MeshPlacement {
            first_cluster: m.first_cluster,
            cluster_count: m.records.len() as u32,
            vertex_base: match &m.residency {
                Residency::Whole(page) => page.vertices.offset,
                Residency::Grouped { .. } => 0,
            },
        })
    }

    /// Resident bytes and the pool's size, for reporting and tests.
    pub fn bytes(&self) -> (u64, u64) {
        self.pool.as_ref().map_or((0, 0), |p| (p.bytes_used(), p.bytes_allocated()))
    }

    #[cfg(test)]
    pub fn resident_groups(&self) -> (usize, usize) {
        (self.resident.iter().filter(|&&r| r).count(), self.resident.len())
    }

    /// Follow the set of converted meshes. Meshes that stay keep their pages;
    /// new ones arrive with only their roots resident (or whole, if skinned);
    /// the scene's cluster list and per-group state are reassembled.
    pub fn sync(&mut self, wanted: &[(MeshKey, Arc<RvgMesh>)], device: &RenderDevice, queue: &RenderQueue) {
        let same = wanted.len() == self.meshes.len() && wanted.iter().zip(&self.meshes).all(|(a, b)| a.0 == b.0);
        if same && self.clusters.is_some() {
            return;
        }
        let pool = self.pool.get_or_insert_with(|| Pool::new(device));
        let mut old: HashMap<MeshKey, StreamedMesh> = self.meshes.drain(..).collect();
        // Room for every new skinned mesh, which uploads whole below, in one
        // growth rather than one per mesh (see `Pool::reserve_more`).
        let (mut whole_vertices, mut whole_words) = (0u64, 0u64);
        for (key, mesh) in wanted {
            if !old.contains_key(key) && mesh.attributes.has(Attributes::SKIN) {
                whole_vertices += mesh.positions.len() as u64;
                whole_words += mesh.cluster_vertices.len() as u64 + (mesh.cluster_triangles.len() as u64).div_ceil(4);
            }
        }
        if whole_vertices > 0 && pool.reserve_more(device, queue, whole_vertices, whole_words) {
            self.generation += 1;
        }
        let mut meshes = Vec::with_capacity(wanted.len());
        for (key, mesh) in wanted {
            let streamed = match old.remove(key) {
                Some(kept) => kept,
                None => {
                    let records = (0..mesh.clusters.len()).map(|i| record(mesh, i)).collect();
                    let residency = if mesh.attributes.has(Attributes::SKIN) {
                        match load_whole(pool, mesh, device, queue) {
                            Some((page, _)) => Residency::Whole(page),
                            None => continue,
                        }
                    } else {
                        Residency::Grouped {
                            graph: mesh.group_graph(),
                            pages: vec![None; mesh.groups.len()],
                        }
                    };
                    let mut streamed = StreamedMesh {
                        mesh: mesh.clone(),
                        residency,
                        records,
                        first_cluster: 0,
                        group_base: 0,
                    };
                    if let Residency::Whole(page) = &streamed.residency {
                        let page = *page;
                        let data = build_whole_offsets(mesh);
                        for (i, (list, tri)) in data.into_iter().enumerate() {
                            point_at(&mut streamed.records[i], &page, list, tri);
                        }
                    }
                    streamed
                }
            };
            meshes.push((*key, streamed));
        }
        for (_, gone) in old {
            free_all(pool, gone);
        }

        // Reassemble the scene-wide numbering.
        self.owner.clear();
        self.pinned.clear();
        self.depth.clear();
        let mut clusters = Vec::new();
        for (index, (_, m)) in meshes.iter_mut().enumerate() {
            m.first_cluster = clusters.len() as u32;
            m.group_base = self.owner.len() as u32;
            let base = m.group_base;
            clusters.extend(m.records.iter().map(|r| GpuCluster {
                group: r.group + base,
                refined: if r.refined == NO_GROUP { NO_GROUP } else { r.refined + base },
                ..*r
            }));
            for (g, group) in m.mesh.groups.iter().enumerate() {
                self.owner.push((index as u32, g as u32));
                self.depth.push(group.depth);
                self.pinned.push(match &m.residency {
                    Residency::Whole(_) => true,
                    Residency::Grouped { graph, .. } => graph.parents[g].is_empty(),
                });
            }
        }
        let groups = self.owner.len();
        self.resident = vec![false; groups];
        self.resident_children = vec![0; groups];
        self.last_drawn = vec![self.frame; groups];
        for (_, m) in &meshes {
            for g in 0..m.mesh.groups.len() {
                let resident = match &m.residency {
                    Residency::Whole(_) => true,
                    Residency::Grouped { pages, .. } => pages[g].is_some(),
                };
                self.resident[(m.group_base + g as u32) as usize] = resident;
            }
            if let Residency::Grouped { graph, pages } = &m.residency {
                for (g, page) in pages.iter().enumerate() {
                    if page.is_some() {
                        for &p in &graph.parents[g] {
                            self.resident_children[(m.group_base + p) as usize] += 1;
                        }
                    }
                }
            }
        }
        self.meshes = meshes;

        self.words = (groups as u32).div_ceil(32).max(1);
        let words = u64::from(self.words) * 4;
        let bits = |label: &'static str, usage: BufferUsages| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: words,
                usage,
                mapped_at_creation: false,
            })
        };
        self.clusters = Some(device.create_buffer(&BufferDescriptor {
            label: Some("vg_clusters"),
            size: (clusters.len().max(1) * std::mem::size_of::<GpuCluster>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        if let Some(buffer) = &self.clusters {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&clusters));
        }
        self.residency = Some(bits("vg_residency", BufferUsages::STORAGE | BufferUsages::COPY_DST));
        self.requests = Some(bits("vg_requests", BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST));
        self.drawn = Some(bits("vg_drawn", BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST));
        // Feedback sized for the old group count would be misread.
        self.feedback.clear();
        self.dirty_clusters.clear();
        self.residency_dirty = true;
        self.generation += 1;

        // Roots of new meshes.
        let roots: Vec<u32> = (0..groups as u32)
            .filter(|&g| self.pinned[g as usize] && !self.resident[g as usize])
            .collect();
        // Room for all of them at once, for the same reason as the skinned
        // meshes above. An upper bound: a vertex shared by two clusters of a
        // page is counted twice.
        let (mut root_vertices, mut root_words) = (0u64, 0u64);
        for &g in &roots {
            let (vertices, words) = self.group_size(g);
            root_vertices += vertices;
            root_words += words;
        }
        if let Some(pool) = self.pool.as_mut() {
            if root_vertices > 0 && pool.reserve_more(device, queue, root_vertices, root_words) {
                self.generation += 1;
            }
        }
        for g in roots {
            self.load(g, device, queue);
        }
        self.flush(queue);
    }

    /// Load one group (its parents must be resident). Returns the bytes
    /// uploaded, or `None` if it could not be placed.
    fn load(&mut self, group: u32, device: &RenderDevice, queue: &RenderQueue) -> Option<u64> {
        let (mesh_index, local) = self.owner[group as usize];
        let pool = self.pool.as_mut()?;
        let (_, m) = &mut self.meshes[mesh_index as usize];
        let Residency::Grouped { graph, pages } = &mut m.residency else {
            return None;
        };
        let range = graph.clusters[local as usize].clone();
        let range = range.start as usize..range.end as usize;
        let data = build_page(&m.mesh, range.clone());
        let bytes = data.bytes();
        let (page, grown) = upload(pool, &data, device, queue)?;
        for (i, &(list, tri)) in range.clone().zip(&data.offsets) {
            point_at(&mut m.records[i], &page, list, tri);
        }
        pages[local as usize] = Some(page);
        let parents = graph.parents[local as usize].clone();
        let first = m.first_cluster;
        let base = m.group_base;
        self.dirty_clusters.push((first + range.start as u32, first + range.end as u32));
        self.resident[group as usize] = true;
        self.last_drawn[group as usize] = self.frame;
        for p in parents {
            self.resident_children[(base + p) as usize] += 1;
        }
        self.residency_dirty = true;
        if grown {
            self.generation += 1;
        }
        Some(bytes)
    }

    fn evict(&mut self, group: u32) {
        let (mesh_index, local) = self.owner[group as usize];
        let Some(pool) = self.pool.as_mut() else {
            return;
        };
        let (_, m) = &mut self.meshes[mesh_index as usize];
        let Residency::Grouped { graph, pages } = &mut m.residency else {
            return;
        };
        if let Some(page) = pages[local as usize].take() {
            pool.free(page);
        }
        for &p in &graph.parents[local as usize] {
            self.resident_children[(m.group_base + p) as usize] -= 1;
        }
        self.resident[group as usize] = false;
        self.residency_dirty = true;
        self.evicted += 1;
    }

    /// Bytes a group's page would take, from its clusters, without building
    /// it: an upper bound (shared vertices counted once per list entry).
    fn estimate(&self, group: u32) -> u64 {
        let (mesh_index, local) = self.owner[group as usize];
        let m = &self.meshes[mesh_index as usize].1;
        let Residency::Grouped { graph, .. } = &m.residency else {
            return 0;
        };
        let range = graph.clusters[local as usize].clone();
        m.mesh.clusters[range.start as usize..range.end as usize]
            .iter()
            .map(|c| u64::from(c.vertex_count) * 32 + u64::from(c.triangle_count) * 3)
            .sum()
    }

    /// Vertices and index words a group's page could take, at most: each
    /// cluster's vertex list, and its triangles' bytes packed four to a word.
    fn group_size(&self, group: u32) -> (u64, u64) {
        let (mesh_index, local) = self.owner[group as usize];
        let m = &self.meshes[mesh_index as usize].1;
        let Residency::Grouped { graph, .. } = &m.residency else {
            return (0, 0);
        };
        let range = graph.clusters[local as usize].clone();
        m.mesh.clusters[range.start as usize..range.end as usize]
            .iter()
            .fold((0, 0), |(vertices, words), c| {
                let list = u64::from(c.vertex_count);
                (vertices + list, words + list + (u64::from(c.triangle_count) * 3).div_ceil(4))
            })
    }

    /// Read whatever feedback has arrived, then load and evict.
    pub fn update(&mut self, budget: u64, device: &RenderDevice, queue: &RenderQueue) {
        self.frame = self.frame.wrapping_add(1);
        let mut requested: Vec<u32> = Vec::new();
        for slot in &mut self.feedback {
            let Some(words) = slot.result.lock().ok().and_then(|mut r| r.take()) else {
                continue;
            };
            slot.busy = false;
            let half = words.len() / 2;
            let (requests, drawn) = words.split_at(half);
            for (w, (&r, &d)) in requests.iter().zip(drawn).enumerate() {
                for bit in 0..32 {
                    let group = (w * 32 + bit) as u32;
                    if group as usize >= self.resident.len() {
                        break;
                    }
                    if d & (1 << bit) != 0 {
                        self.last_drawn[group as usize] = self.frame;
                    }
                    if r & (1 << bit) != 0 && !self.resident[group as usize] {
                        requested.push(group);
                    }
                }
            }
        }
        if !requested.is_empty() {
            requested.sort_unstable();
            requested.dedup();
            // Coarse first: they unlock the finer ones, and fix the most
            // visible error per byte.
            requested.sort_by_key(|&g| std::cmp::Reverse(self.depth[g as usize]));
            let mut uploaded = 0u64;
            let mut loaded_on_request = Vec::new();
            'requests: for group in requested.iter().copied() {
                // Parents before children, so residency stays closed.
                let mut chain = vec![group];
                let mut i = 0;
                while i < chain.len() {
                    let g = chain[i];
                    let (mesh_index, local) = self.owner[g as usize];
                    let m = &self.meshes[mesh_index as usize].1;
                    if let Residency::Grouped { graph, .. } = &m.residency {
                        for &p in &graph.parents[local as usize] {
                            let p = m.group_base + p;
                            if !self.resident[p as usize] && !chain.contains(&p) {
                                chain.push(p);
                            }
                        }
                    }
                    i += 1;
                }
                for &g in chain.iter().rev() {
                    if self.resident[g as usize] {
                        continue;
                    }
                    if uploaded >= UPLOAD_BUDGET {
                        break 'requests;
                    }
                    let need = self.estimate(g);
                    if !self.make_room(need, budget) {
                        break 'requests;
                    }
                    match self.load(g, device, queue) {
                        Some(bytes) => uploaded += bytes,
                        None => break 'requests,
                    }
                }
                loaded_on_request.push(group);
            }
            self.prefetch_below(loaded_on_request, uploaded, budget, device, queue);
        }
        self.flush(queue);
        let (used, allocated) = self.bytes();
        let step = 16 << 20;
        if used / step != self.reported.0 / step || allocated != self.reported.1 {
            self.reported = (used, allocated);
            debug!(
                "[virtual geometry] {} MB resident of a {} MB budget ({} MB allocated)",
                used >> 20,
                budget >> 20,
                allocated >> 20
            );
        }
    }

    /// Load the finer groups below those just loaded on request, coarse first,
    /// with whatever is left of this frame's upload budget and only into free
    /// memory: nothing is evicted for them.
    ///
    /// The cull pass can only ask for the next level below what is resident,
    /// since it judges a group by clusters it has, so detail used to arrive a
    /// level per feedback round trip, two or three frames each. A model that
    /// had just loaded took tens of seconds to reach full detail in a large
    /// scene, drawn meanwhile from coarse levels that Solari, tracing the full
    /// detail, did not match: bright and black chunks over everything until
    /// it settled. A group asked for is a region the camera wants finer, so
    /// the levels below it are likely wanted too; one that is not is never
    /// drawn, so it ages out and is evicted like any other group once the
    /// memory is needed.
    fn prefetch_below(&mut self, loaded: Vec<u32>, mut uploaded: u64, budget: u64, device: &RenderDevice, queue: &RenderQueue) {
        let mut frontier = loaded;
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for group in frontier {
                let (mesh_index, local) = self.owner[group as usize];
                let m = &self.meshes[mesh_index as usize].1;
                let Residency::Grouped { graph, .. } = &m.residency else { continue };
                for &child in &graph.children[local as usize] {
                    let child_parents_resident = graph.parents[child as usize]
                        .iter()
                        .all(|&p| self.resident[(m.group_base + p) as usize]);
                    let child = m.group_base + child;
                    if !self.resident[child as usize] && child_parents_resident && !next.contains(&child) {
                        next.push(child);
                    }
                }
            }
            for &group in &next {
                if uploaded >= UPLOAD_BUDGET {
                    return;
                }
                let need = self.estimate(group);
                if self.bytes().0 + need > budget {
                    return;
                }
                match self.load(group, device, queue) {
                    Some(bytes) => uploaded += bytes,
                    None => return,
                }
            }
            frontier = next;
        }
    }

    /// Evict until `need` more bytes fit in `budget`. Only groups nothing
    /// resident depends on and not drawn lately go; false if that is not
    /// enough.
    fn make_room(&mut self, need: u64, budget: u64) -> bool {
        let used = self.bytes().0;
        if used + need <= budget {
            return true;
        }
        let mut candidates: Vec<u32> = (0..self.resident.len() as u32)
            .filter(|&g| {
                let g = g as usize;
                self.resident[g]
                    && !self.pinned[g]
                    && self.resident_children[g] == 0
                    && self.frame.wrapping_sub(self.last_drawn[g]) > KEEP_FRAMES
            })
            .collect();
        candidates.sort_by_key(|&g| self.last_drawn[g as usize]);
        let mut queue: std::collections::VecDeque<u32> = candidates.into();
        while self.bytes().0 + need > budget {
            let Some(g) = queue.pop_front() else {
                return false;
            };
            // A candidate's children may have been evicted since it was
            // listed only in its favour; one that gained none stays valid.
            if !self.resident[g as usize] || self.resident_children[g as usize] != 0 {
                continue;
            }
            self.evict(g);
            // Its parents may now be childless; they join the back of the
            // line, being at least as recently drawn.
            let (mesh_index, local) = self.owner[g as usize];
            let m = &self.meshes[mesh_index as usize].1;
            if let Residency::Grouped { graph, .. } = &m.residency {
                for &p in &graph.parents[local as usize] {
                    let p = m.group_base + p;
                    let pu = p as usize;
                    if self.resident[pu]
                        && !self.pinned[pu]
                        && self.resident_children[pu] == 0
                        && self.frame.wrapping_sub(self.last_drawn[pu]) > KEEP_FRAMES
                    {
                        queue.push_back(p);
                    }
                }
            }
        }
        true
    }

    /// Write changed cluster records and the residency bits.
    fn flush(&mut self, queue: &RenderQueue) {
        if let Some(buffer) = &self.clusters {
            for (start, end) in self.dirty_clusters.drain(..) {
                // Find the mesh owning this range to read its local records.
                let Some((_, m)) = self
                    .meshes
                    .iter()
                    .find(|(_, m)| start >= m.first_cluster && end <= m.first_cluster + m.records.len() as u32)
                else {
                    continue;
                };
                let base = m.group_base;
                let local = &m.records[(start - m.first_cluster) as usize..(end - m.first_cluster) as usize];
                let records: Vec<GpuCluster> = local
                    .iter()
                    .map(|r| GpuCluster {
                        group: r.group + base,
                        refined: if r.refined == NO_GROUP { NO_GROUP } else { r.refined + base },
                        ..*r
                    })
                    .collect();
                queue.write_buffer(
                    buffer,
                    u64::from(start) * std::mem::size_of::<GpuCluster>() as u64,
                    bytemuck::cast_slice(&records),
                );
            }
        }
        if self.residency_dirty {
            self.residency_dirty = false;
            if let Some(buffer) = &self.residency {
                let mut words = vec![0u32; self.words as usize];
                for (g, &r) in self.resident.iter().enumerate() {
                    if r {
                        words[g / 32] |= 1 << (g % 32);
                    }
                }
                queue.write_buffer(buffer, 0, bytemuck::cast_slice(&words));
            }
        }
    }

    /// After the frame's commands are submitted: copy this frame's requests
    /// and drawn bits to a free staging buffer, clear them for the next frame,
    /// and map the copy.
    pub fn read_back(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        let (Some(requests), Some(drawn)) = (&self.requests, &self.drawn) else {
            return;
        };
        let bytes = u64::from(self.words) * 4;
        if self.feedback.is_empty() {
            self.feedback = (0..FEEDBACK_SLOTS)
                .map(|_| FeedbackSlot {
                    staging: device.create_buffer(&BufferDescriptor {
                        label: Some("vg_feedback"),
                        size: bytes * 2,
                        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    busy: false,
                    result: Arc::default(),
                })
                .collect();
        }
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("vg_feedback"),
        });
        let slot = (0..FEEDBACK_SLOTS)
            .map(|i| (self.next_slot + i) % FEEDBACK_SLOTS)
            .find(|&i| !self.feedback[i].busy);
        if let Some(i) = slot {
            let staging = &self.feedback[i].staging;
            encoder.copy_buffer_to_buffer(requests, 0, staging, 0, bytes);
            encoder.copy_buffer_to_buffer(drawn, 0, staging, bytes, bytes);
        }
        // Cleared every frame whether or not a slot was free: a request left
        // set would be read again a frame later as if it were new.
        encoder.clear_buffer(requests, 0, None);
        encoder.clear_buffer(drawn, 0, None);
        queue.submit([encoder.finish()]);
        let Some(i) = slot else {
            return;
        };
        self.next_slot = (i + 1) % FEEDBACK_SLOTS;
        let feedback = &mut self.feedback[i];
        feedback.busy = true;
        let buffer = feedback.staging.clone();
        let result = feedback.result.clone();
        feedback.staging.slice(..).map_async(MapMode::Read, move |mapped| {
            let words = mapped.ok().and_then(|()| {
                let words = buffer
                    .slice(..)
                    .get_mapped_range()
                    .ok()
                    .map(|data| bytemuck::cast_slice::<u8, u32>(&data).to_vec());
                buffer.unmap();
                words
            });
            if let Ok(mut slot) = result.lock() {
                // A failed map still frees the slot, with no requests.
                *slot = Some(words.unwrap_or_default());
            }
        });
    }
}

/// Per cluster of a whole mesh: its vertex list offset and its triangles'
/// byte offset within one page laid out as `build_page` would.
fn build_whole_offsets(mesh: &RvgMesh) -> Vec<(u32, u32)> {
    let triangle_base = mesh.cluster_vertices.len() as u32 * 4;
    mesh.clusters
        .iter()
        .map(|c| (c.vertex_offset, triangle_base + c.triangle_offset * 3))
        .collect()
}

/// A whole mesh as one page, with its original vertex numbering (the skin
/// pass and the skin data index vertices by it).
fn load_whole(pool: &mut Pool, mesh: &RvgMesh, device: &RenderDevice, queue: &RenderQueue) -> Option<(Page, bool)> {
    let mut attributes = Vec::with_capacity(mesh.positions.len() * 4);
    pack_attributes(mesh, &mut attributes);
    let mut words = mesh.cluster_vertices.clone();
    words.extend(mesh.cluster_triangles.chunks(4).map(|w| {
        let mut word = [0u8; 4];
        word[..w.len()].copy_from_slice(w);
        u32::from_le_bytes(word)
    }));
    let data = PageData {
        positions: mesh.positions.as_flattened().to_vec(),
        attributes,
        words,
        offsets: Vec::new(),
    };
    upload(pool, &data, device, queue)
}

fn upload(pool: &mut Pool, data: &PageData, device: &RenderDevice, queue: &RenderQueue) -> Option<(Page, bool)> {
    let vertices = (data.positions.len() / 3) as u32;
    let (page, grown) = pool.allocate(device, queue, vertices, data.words.len() as u32)?;
    if vertices > 0 {
        queue.write_buffer(&pool.positions, u64::from(page.vertices.offset) * 12, bytemuck::cast_slice(&data.positions));
        queue.write_buffer(&pool.attributes, u64::from(page.vertices.offset) * 16, bytemuck::cast_slice(&data.attributes));
    }
    if !data.words.is_empty() {
        queue.write_buffer(&pool.indices, u64::from(page.words.offset) * 4, bytemuck::cast_slice(&data.words));
    }
    Some((page, grown))
}

/// Point a cluster's record into the page holding it.
fn point_at(record: &mut GpuCluster, page: &Page, list: u32, triangles: u32) {
    record.vertex_base = page.vertices.offset;
    record.vertex_offset = page.words.offset + list;
    record.triangle_offset = page.words.offset * 4 + triangles;
}

fn free_all(pool: &mut Pool, mesh: StreamedMesh) {
    match mesh.residency {
        Residency::Whole(page) => pool.free(page),
        Residency::Grouped { pages, .. } => {
            for page in pages.into_iter().flatten() {
                pool.free(page);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a page back the way the shaders do (`vg_triangle_vertex`: the
    /// triangle byte, then the page-local list, then the page's vertices) and
    /// check every corner lands on the same position as in the mesh itself.
    /// A page that renumbered its vertices wrongly would draw triangles
    /// across the wrong corners.
    fn check_pages(mesh: &RvgMesh) {
        let graph = mesh.group_graph();
        for (g, range) in graph.clusters.iter().enumerate() {
            let range = range.start as usize..range.end as usize;
            let page = build_page(mesh, range.clone());
            for (k, i) in range.enumerate() {
                let c = &mesh.clusters[i];
                let (list, triangles) = page.offsets[k];
                for t in 0..c.triangle_count * 3 {
                    let byte = triangles + t;
                    let local = (page.words[(byte / 4) as usize] >> ((byte % 4) * 8)) & 0xff;
                    let vertex = page.words[(list + local) as usize] as usize;
                    let got = &page.positions[vertex * 3..vertex * 3 + 3];
                    let original_local = mesh.cluster_triangles[(c.triangle_offset * 3 + t) as usize] as usize;
                    let original = mesh.cluster_vertices[c.vertex_offset as usize + original_local] as usize;
                    assert_eq!(
                        got,
                        &mesh.positions[original][..],
                        "group {g}, cluster {i}, corner {t}: the page points at the wrong vertex"
                    );
                }
            }
        }
    }

    #[test]
    fn pages_reproduce_their_clusters() {
        let mut input = renzora_rvg::bake::MeshInput::default();
        let (rings, segments) = (48u32, 96u32);
        input.positions.push([0.0, 1.0, 0.0]);
        for r in 1..rings {
            let phi = std::f32::consts::PI * r as f32 / rings as f32;
            for s in 0..segments {
                let theta = std::f32::consts::TAU * s as f32 / segments as f32;
                input.positions.push([phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()]);
            }
        }
        input.positions.push([0.0, -1.0, 0.0]);
        let ring = |r: u32, s: u32| 1 + (r - 1) * segments + s % segments;
        let bottom = input.positions.len() as u32 - 1;
        for s in 0..segments {
            input.indices.extend([0, ring(1, s + 1), ring(1, s)]);
            input.indices.extend([bottom, ring(rings - 1, s), ring(rings - 1, s + 1)]);
        }
        for r in 1..rings - 1 {
            for s in 0..segments {
                let (a, b, c, d) = (ring(r, s), ring(r, s + 1), ring(r + 1, s), ring(r + 1, s + 1));
                input.indices.extend([a, b, d, a, d, c]);
            }
        }
        input.normals = input.positions.clone();
        check_pages(&renzora_rvg::bake::build_mesh(&input).unwrap());
    }

    /// The same over a real converted model, when one is named:
    /// `RENZORA_RVG_CHECK=path/to/model.rvg`.
    #[test]
    fn pages_reproduce_a_real_model() {
        let Ok(path) = std::env::var("RENZORA_RVG_CHECK") else {
            return;
        };
        let file = renzora_rvg::RvgFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        for mesh in &file.meshes {
            check_pages(mesh);
        }
    }
}
