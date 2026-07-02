//! This module contains structs and functions that create and manage render-world entities and GPU buffers.
//!
//! Lights and Occluders are stored in global buffers through their own [`BufferManager`]s.
//!
//! Round and Polygonal Occluders are stores in separate buffers due to having significantly different structures.   
//!
//! Vertices for Polygonal Occluders are stored in a global [`VertexBuffer`].

use core::f32;
use std::{
    array,
    collections::{BinaryHeap, VecDeque},
    f32::consts::{PI, TAU},
};

use bevy::{
    platform::collections::HashMap,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        render_resource::{
            BindingResource, BufferUsages, RawBufferVec, ShaderType, StorageBuffer,
            encase::private::WriteInto,
        },
        renderer::{RenderDevice, RenderQueue},
        view::RetainedViewEntity,
    },
};
use bytemuck::{NoUninit, Pod, Zeroable};

use crate::{
    lights::{ExtractedPointLight, Falloff, LightIndex, UniformPointLight},
    occluders::{
        ExtractedOccluder, Occluder2dShape, PolyOccluderIndex, RoundOccluderIndex, UniformOccluder,
        UniformRoundOccluder,
    },
    visibility::NotVisible,
};

/// Plugin that adds systems and observers for managing GPU buffers. This is added automatically through [`FireflyPlugin`](crate::prelude::FireflyPlugin)
pub struct BuffersPlugin;

impl Plugin for BuffersPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(RenderStartup, spawn_observers);
        render_app.add_systems(
            Render,
            (prepare_occluders, prepare_lights)
                .in_set(RenderSystems::Prepare)
                .before(crate::prepare::prepare_data),
        );

        render_app.add_systems(Render, handle_not_visible_entities);
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<BufferManager<UniformRoundOccluder>>();
        render_app.init_resource::<BufferManager<UniformOccluder>>();
        render_app.init_resource::<BufferManager<UniformPointLight>>();
        render_app.init_resource::<VertexBuffer>();
    }
}

fn spawn_observers(mut commands: Commands) {
    commands.spawn(Observer::new(on_occluder_removed));
    commands.spawn(Observer::new(on_light_removed));
}

// handles buffer when the light gets despawned or the component is removed
fn on_light_removed(
    trigger: On<Remove, ExtractedPointLight>,
    mut lights: Query<&mut LightIndex>,
    mut light_manager: ResMut<BufferManager<UniformPointLight>>,
) {
    if let Ok(mut index) = lights.get_mut(trigger.entity)
        && let Some(old_index) = index.0
    {
        light_manager.free_index(old_index);
        index.0 = None;
    }
}

// handles buffer when the occluder gets despawned or the component is removed
fn on_occluder_removed(
    trigger: On<Remove, ExtractedOccluder>,
    mut occluders: Query<
        (
            &ExtractedOccluder,
            &mut RoundOccluderIndex,
            &mut PolyOccluderIndex,
        ),
        With<ExtractedOccluder>,
    >,
    mut round_manager: ResMut<BufferManager<UniformRoundOccluder>>,
    mut poly_manager: ResMut<BufferManager<UniformOccluder>>,
    mut vertex_buffer: ResMut<VertexBuffer>,
) {
    if let Ok((occluder, mut round_index, mut poly_index)) = occluders.get_mut(trigger.entity) {
        if matches!(occluder.shape, Occluder2dShape::RoundRectangle { .. }) {
            if let Some(old_index) = round_index.0 {
                round_manager.free_index(old_index);
                round_index.0 = None;
            }
        } else {
            if let Some(old_index) = poly_index.occluder {
                poly_manager.free_index(old_index);
                poly_index.occluder = None;
            }
            if let Some(old_index) = poly_index.vertices {
                vertex_buffer.free_indices(occluder.shape.n_vertices(), old_index.generation);
                poly_index.vertices = None;
            }
        }
    }
}

// handles buffer when entity is not visible anymore
fn handle_not_visible_entities(
    mut occluders: Query<
        (
            Entity,
            &ExtractedOccluder,
            &mut RoundOccluderIndex,
            &mut PolyOccluderIndex,
        ),
        With<NotVisible>,
    >,
    mut lights: Query<(Entity, &mut LightIndex), With<NotVisible>>,
    mut round_manager: ResMut<BufferManager<UniformRoundOccluder>>,
    mut poly_manager: ResMut<BufferManager<UniformOccluder>>,
    mut vertex_buffer: ResMut<VertexBuffer>,
    mut light_manager: ResMut<BufferManager<UniformPointLight>>,
    mut commands: Commands,
) {
    for (id, occluder, mut round_index, mut poly_index) in &mut occluders {
        if matches!(occluder.shape, Occluder2dShape::RoundRectangle { .. }) {
            if let Some(old_index) = round_index.0 {
                round_manager.free_index(old_index);
                round_index.0 = None;
            }
        } else {
            if let Some(old_index) = poly_index.occluder {
                poly_manager.free_index(old_index);
                poly_index.occluder = None;
            }
            if let Some(old_index) = poly_index.vertices {
                vertex_buffer.free_indices(occluder.shape.n_vertices(), old_index.generation);
                poly_index.vertices = None;
            }
        }

        commands.entity(id).remove::<ExtractedOccluder>();
        commands.entity(id).remove::<NotVisible>();
    }

    for (id, mut index) in &mut lights {
        if let Some(old_index) = index.0 {
            light_manager.free_index(old_index);
            index.0 = None;
        }

        commands.entity(id).remove::<ExtractedPointLight>();
        commands.entity(id).remove::<NotVisible>();
    }
}

// adds lights to buffer for use in prepare system
fn prepare_lights(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut lights: Query<(&ExtractedPointLight, &mut LightIndex)>,
    mut light_manager: ResMut<BufferManager<UniformPointLight>>,
) {
    for (light, mut index) in &mut lights {
        let changed = light.changes.0;

        let light = UniformPointLight {
            pos: light.pos,
            intensity: light.intensity,
            radius: light.radius,
            color: light.color.to_linear().to_vec4(),
            z: light.z,
            core_radius: light.core.radius,
            core_boost: light.core.boost,
            core_falloff: match light.core.falloff {
                Falloff::InverseSquare { .. } => 0,
                Falloff::Linear { .. } => 1,
                Falloff::None => 2,
            },
            core_falloff_intensity: light.core.falloff.intensity(),
            falloff: match light.falloff {
                Falloff::InverseSquare { .. } => 0,
                Falloff::Linear { .. } => 1,
                Falloff::None => 2,
            },
            falloff_intensity: light.falloff.intensity(),
            inner_angle: light.angle.inner / 180. * PI,
            outer_angle: light.angle.outer / 180. * PI,
            dir: light.dir,
            height: light.height,
        };

        let new_index =
            light_manager.set_value(&light, index.0, changed, &render_device, &render_queue);
        index.0 = Some(new_index);
    }

    light_manager.flush(&render_device, &render_queue);
}

// adds occluders to buffers for use in prepare system
fn prepare_occluders(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut occluders: Query<(
        &ExtractedOccluder,
        &mut RoundOccluderIndex,
        &mut PolyOccluderIndex,
    )>,
    mut round_manager: ResMut<BufferManager<UniformRoundOccluder>>,
    mut poly_manager: ResMut<BufferManager<UniformOccluder>>,
    mut vertex_buffer: ResMut<VertexBuffer>,
) {
    for (occluder, mut round_index, mut poly_index) in &mut occluders {
        let changed = occluder.changes.0;
        if let Occluder2dShape::RoundRectangle {
            half_width,
            half_height,
            radius,
        } = occluder.shape
        {
            let value = UniformRoundOccluder {
                pos: occluder.pos,
                rot: occluder.rot,
                half_width,
                half_height,
                radius,
                // padding: default(),
                z: occluder.z,
                color: occluder.color.to_linear().to_vec4(),
                opacity: occluder.opacity,
                z_sorting: match occluder.z_sorting {
                    true => 1,
                    false => 0,
                },
                _pad1: [0, 0, 0],
            };

            // assert_eq!(std::mem::size_of::<UniformRoundOccluder>(), 64);
            // assert_eq!(std::mem::align_of::<UniformRoundOccluder>(), 16);

            let new_index = round_manager.set_value(
                &value,
                round_index.0,
                changed,
                &render_device,
                &render_queue,
            );
            round_index.0 = Some(new_index);
        } else {
            let vertex_index = vertex_buffer.write_vertices(
                occluder,
                poly_index.vertices,
                &render_device,
                &render_queue,
                changed,
            );
            poly_index.vertices = Some(vertex_index);

            let value = UniformOccluder {
                vertex_start: vertex_index.index as u32,
                n_vertices: occluder.shape.n_vertices(),
                z: occluder.z,
                color: occluder.color.to_linear().to_vec4(),
                opacity: occluder.opacity,
                z_sorting: match occluder.z_sorting {
                    true => 1,
                    false => 0,
                },
                _pad1: [0, 0, 0],
            };

            let new_index = poly_manager.set_value(
                &value,
                poly_index.occluder,
                changed,
                &render_device,
                &render_queue,
            );
            poly_index.occluder = Some(new_index);
        }
    }

    round_manager.flush(&render_device, &render_queue);
    poly_manager.flush(&render_device, &render_queue);
    vertex_buffer.pass(&render_device, &render_queue);
}

/// The max number of elements that will be written in a single command by [`BufferManager`].
const MAX_SINGLE_WRITE_LENGTH: usize = 64;

/// This resource is a wrapper around [`RawBufferVec`] that reserves and distributes VRAM slots to
/// a set of entities that are intended to be transferred to the GPU. It is currently used for Occluders and Lights.
#[derive(Resource)]
pub struct BufferManager<T: ShaderType + WriteInto + Default + NoUninit> {
    buffer: RawBufferVec<T>,
    next_index: usize,
    free_indices: VecDeque<usize>,
    write_min: usize,
    write_max: usize,
    current_generation: u32,
}

impl<T: ShaderType + WriteInto + Default + NoUninit> FromWorld for BufferManager<T> {
    fn from_world(world: &mut bevy::prelude::World) -> BufferManager<T> {
        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();

        Self::new(device, queue)
    }
}

impl<T: ShaderType + WriteInto + Default + NoUninit> BufferManager<T> {
    fn new_index(&mut self) -> usize {
        self.free_indices.pop_back().unwrap_or_else(|| {
            self.next_index += 1;
            self.next_index - 1
        })
    }

    fn new(device: &RenderDevice, queue: &RenderQueue) -> Self {
        let mut res = Self {
            buffer: RawBufferVec::<T>::new(BufferUsages::STORAGE),
            next_index: 2,
            free_indices: default(),
            write_min: usize::MAX,
            write_max: usize::MIN,
            current_generation: 0,
        };

        res.buffer.set_label("global buffer".into());

        // empty value is added so the buffer can be written to VRAM from the start
        res.buffer.push(default());
        res.buffer.push(default());
        res.buffer.write_buffer(device, queue);

        res
    }

    /// Get the binding of this buffer. It is guaranteed to exist.
    pub fn binding(&self) -> BindingResource<'_> {
        self.buffer.binding().unwrap()
    }

    /// Called by an entity to pass it's current index and value to the buffer.
    /// It returns back it's (possibly changed) index.  
    ///
    /// It is an entity's responsibility to store the received index and use it in subsequent calls.
    ///
    /// If an entity didn't have any changes, it shouldn't call this.
    pub fn set_value(
        &mut self,
        value: &T,
        index: Option<BufferIndex>,
        changed: bool,
        device: &RenderDevice,
        queue: &RenderQueue,
    ) -> BufferIndex {
        if !changed
            && let Some(index) = index
            && index.generation == self.current_generation
        {
            return index;
        }

        let index = match index {
            None => self.new_index(),
            Some(BufferIndex { index, generation }) => {
                if index < self.next_index && generation == self.current_generation {
                    index
                } else {
                    self.new_index()
                }
            }
        };

        if index >= self.buffer.len() {
            self.buffer.push(*value);
        } else {
            self.buffer.set(index as u32, *value);
        }

        let next_min = self.write_min.min(index);
        let next_max = self.write_max.max(index);

        if next_max - next_min > MAX_SINGLE_WRITE_LENGTH {
            self.write(device, queue);

            self.write_min = index;
            self.write_max = index;
        } else {
            self.write_min = next_min;
            self.write_max = next_max;
        }

        BufferIndex {
            index,
            generation: self.current_generation,
        }
    }

    fn write(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        if self.write_min != usize::MAX {
            if self.write_max >= self.buffer.capacity() {
                self.buffer.reserve(
                    ((self.write_max + 1) as f32 / 1024.0).ceil() as usize * 1024,
                    device,
                );
                self.buffer.write_buffer(device, queue);
            } else {
                self.buffer
                    .write_buffer_range(queue, self.write_min..(self.write_max + 1))
                    .expect("couldn't write to buffer");
            }
        }

        // info!(
        //     "Finished writing! Buffer length: {}, Element size: {}, Buffer size: {}, Buffer capacity: {}, Unoccupied: {}",
        //     self.buffer.len(),
        //     T::min_size().get(),
        //     self.buffer.buffer().unwrap().size(),
        //     self.buffer.capacity(),
        //     self.free_indices.len(),
        // );
    }

    /// Flush the changes at the end of a render frame. This writes all changes to the GPU.
    pub fn flush(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        self.write(device, queue);

        // Refragmentation. Because of wasted space the buffer will empty itself and pass all-new data next frame. This can be optimized
        if self.free_indices.len() > 500 && self.free_indices.len() > self.buffer.capacity() / 2 {
            let old_generation = self.current_generation;
            *self = Self::new(device, queue);
            self.current_generation = old_generation + 1;
        }

        self.write_min = usize::MAX;
        self.write_max = usize::MIN;
    }

    /// An entity that has gone out of view, been despawned, or is no longer intended to be rendered,
    /// has to call this method to free it's Buffer slot.
    ///
    /// The index / slot will be automatically redistributed to another entity when needed.
    pub fn free_index(&mut self, index: BufferIndex) {
        if index.generation != self.current_generation {
            return;
        };

        if index.index >= self.buffer.len() {
            return;
        }

        self.free_indices.push_front(index.index);
    }
}

/// The amount of bins that each [`Bins`] will have.
pub const N_BINS: usize = 256;
pub const N_BINS_FLOAT: f32 = 256.0;

/// A component that each light has, containing the [BinBuffer]s for each camera view.
#[derive(Component, Default)]
pub struct BinBuffers(pub HashMap<RetainedViewEntity, BinBuffer>);

/// A struct containing sets of bins of occluders for faster iteration.
/// This is the most important acceleration structure used by Firefly. It is used in a custom
/// type of angular sweep with BVH-inspired elements.
pub struct BinBuffer {
    /// List of all Occluders that will be written to the GPU.
    buffer: RawBufferVec<OccluderPointer>,
    /// Indices describing where each bin starts, written to the GPU. The extra value at the end is the maximum index / length.  
    bin_indices: StorageBuffer<BinIndices>,
    /// Data stored on the CPU.
    occluders: [BinaryHeap<OccluderPointer>; N_BINS],
}

/// Wrapper for the bin indices, so it can impl Default.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, ShaderType)]
pub struct BinIndices {
    indices: [u32; N_BINS + 1],
}

impl Default for BinIndices {
    fn default() -> Self {
        BinIndices {
            indices: [0; N_BINS + 1],
        }
    }
}

impl Default for BinBuffer {
    fn default() -> Self {
        Self {
            buffer: RawBufferVec::<OccluderPointer>::new(BufferUsages::STORAGE),
            bin_indices: StorageBuffer::<BinIndices>::default(),
            occluders: array::from_fn(|_| default()),
        }
    }
}

impl BinBuffer {
    /// Get the binding of the bins. It is guaranteed to exist.
    pub fn bin_binding(&self) -> BindingResource<'_> {
        self.buffer.binding().unwrap()
    }

    /// Get the binding of the end index of each bin. It is guaranteed to exist.
    pub fn bin_indices_binding(&self) -> BindingResource<'_> {
        self.bin_indices.binding().unwrap()
    }

    /// Write this buffer's data to the GPU. This function also sorts the
    /// occluders by distance enabling early-stopping in GPU checks.
    pub fn write(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        let mut bin_indices = [0; N_BINS + 1];

        let mut count = 1;

        let values = self.buffer.values_mut();

        for (index, bin) in self.occluders.iter_mut().enumerate() {
            bin_indices[index] = count as u32;
            count += bin.len();
            // info!("{:?}", &bin.clone().into_sorted_vec());
            // values.extend_from_slice(&bin.clone().into_sorted_vec());

            loop {
                let Some(x) = bin.pop() else { break };
                values.push(x);
            }
        }
        bin_indices[N_BINS] = count as u32;

        self.buffer.write_buffer(device, queue);

        self.bin_indices.set(BinIndices {
            indices: bin_indices,
        });
        self.bin_indices.write_buffer(device, queue);
    }

    /// Clear the buffer and add one empty set of bins.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.buffer.push(OccluderPointer::default());

        for bin in self.occluders.iter_mut() {
            bin.clear();
        }
    }

    // const SCALE: f32 = N_BINS_FLOAT / TAU;
    /// Add an occluder to this buffer. Or a set of edges, in case of a polygonal occluder.
    pub fn add_occluder(&mut self, data: &OccluderData) {
        if data.angle.ceil() >= TAU {
            self.add_to_bins(0, N_BINS - 1, data.pointer);
            return;
        }

        // info!("init min angle: {}", edge.min_angle);

        let min_angle = if data.min_angle < -PI {
            data.min_angle + TAU
        } else {
            data.min_angle
        };

        let min_bin = (((min_angle + PI) / TAU) * N_BINS_FLOAT).floor() as usize;
        let n_bins = ((data.angle / TAU) * N_BINS_FLOAT).ceil() as usize;

        // info!("min bin: {min_bin}, n_bins: {n_bins}");

        // self.add_to_bins(0, N_BINS - 1, edge.pointer);
        if min_bin + n_bins >= N_BINS {
            self.add_to_bins(min_bin, N_BINS - 1, data.pointer);
            self.add_to_bins(0, min_bin + n_bins - N_BINS, data.pointer);
        } else {
            self.add_to_bins(min_bin, min_bin + n_bins, data.pointer);
        }
    }

    fn add_to_bins(&mut self, min_bin: usize, max_bin: usize, pointer: OccluderPointer) {
        // info!("writing buffers {min_bin} to {max_bin}");
        for index in min_bin..(max_bin + 1) {
            self.occluders[index].push(pointer);
        }
    }
}

/// CPU struct describing an occluder or edge.
#[derive(Clone)]
pub struct OccluderData {
    pub pointer: OccluderPointer,
    pub min_angle: f32,
    pub angle: f32,
}

/// Compact struct pointing to a round occluder, or a chain of vertices from a polygonal occluder.  
#[repr(C)]
#[derive(Default, Pod, Zeroable, Clone, Copy, ShaderType, Debug)]
pub struct OccluderPointer {
    /// The index's first bit is the type of occluder: 0 for round, 1 for polygonal.
    pub index: u32,
    /// The index of the first vertex of the path in the global vertex buffer, in case the occluder is polygonal.
    ///
    /// There is also additional information encoded at the left of this value:
    ///
    /// - A `term` variable that takes 2 bits, describing the terminator format of this chain. This is 1
    /// if the chain ends looping over the atan2 seam, 2 if it starts like that, and 0 otherwise.
    ///
    /// - A `rev` variable that takes 1 bit and specifies if the chain is made of vertices in the same order as they're
    /// stored in (clockwise) or not. This is used for when a light is inside the perimeter of an occluder and the
    /// edges need to be reversed.
    pub min_v: u32,
    /// In case this edge loops over the atan2 seam, this will dicate the length after which that happens.
    pub split: u32,
    /// The length of the vertex edge, in case the occluder is polygonal.
    pub length: u32,
    /// The minimum distance from the occluder to the light source. This is used to accelerate GPU computations,
    /// because a point can't be blocked by this occluder if it's distance is greater than the point's own
    /// distance to the light source.
    pub distance: f32,
}

impl PartialEq for OccluderPointer {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for OccluderPointer {}

impl PartialOrd for OccluderPointer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (-self.distance).partial_cmp(&-other.distance)
    }
}

impl Ord for OccluderPointer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (-self.distance).total_cmp(&-other.distance)
    }
}

/// A global buffer in which all visible vertices are stored.
///
/// This is different from the [`BufferManager`] in order to use a specific allocation
/// that suits vertices better. They are quickly added on top of each other without keeping track
/// of their position for re-allocation. When an occluder disappears, it's number of vertices is simply
/// subtracted from the total lenght of the buffer, and the buffer refragments itself when
/// there is a significant amount of wasted space.  
#[derive(Resource)]
pub struct VertexBuffer {
    vertices: RawBufferVec<Vec2>,
    next_index: usize,
    empty_slots: u32,
    current_generation: u32,
}

impl FromWorld for VertexBuffer {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();

        Self::new(device, queue)
    }
}

impl VertexBuffer {
    fn new(device: &RenderDevice, queue: &RenderQueue) -> Self {
        let mut res = Self {
            vertices: RawBufferVec::<Vec2>::new(BufferUsages::STORAGE),
            next_index: 1,
            empty_slots: 0,
            current_generation: 0,
        };

        res.vertices.set_label("vertex buffer".into());

        // empty value is added so the buffer can be written to VRAM from the start

        res.vertices.push(default());
        res.vertices.write_buffer(device, queue);

        res
    }

    /// Get the binding of this buffer. It is guaranteed to exist.
    pub fn binding(&self) -> BindingResource<'_> {
        self.vertices.binding().unwrap()
    }

    /// Insert all of an occluder's vertices to this buffer. This
    /// function also automatically writes them to the GPU.  
    pub fn write_vertices(
        &mut self,
        occluder: &ExtractedOccluder,
        index: Option<BufferIndex>,
        device: &RenderDevice,
        queue: &RenderQueue,
        changed: bool,
    ) -> BufferIndex {
        if !changed
            && let Some(index) = index
            && index.generation == self.current_generation
        {
            return index;
        }

        let index = match index {
            None => self.next_index,
            Some(BufferIndex { index, generation }) => {
                if index < self.next_index && generation == self.current_generation {
                    index
                } else {
                    self.next_index
                }
            }
        };

        // change existent vertices
        if index < self.next_index {
            let mut last_index = index;
            for vertex in occluder.vertices_iter() {
                if last_index >= self.vertices.len() {
                    self.vertices.push(vertex);
                    warn!("hmm.. what?");
                } else {
                    self.vertices.set(last_index as u32, vertex);
                }

                last_index += 1;
            }

            self.vertices
                .write_buffer_range(queue, index..last_index)
                .expect("couldn't write range");

            // for vertex in self.vertices.values() {
            //     info!("{vertex}");
            // }

            return BufferIndex {
                index,
                generation: self.current_generation,
            };
        }

        // add new vertices
        for vertex in occluder.vertices_iter() {
            self.vertices.push(vertex);
            self.next_index += 1;
        }

        // if self.next_index % 2 == 1 {
        //     self.vertices.push(default());
        //     self.next_index += 1;
        // }

        if self.next_index >= self.vertices.capacity() {
            self.vertices.reserve(
                (self.next_index as f32 / 4096.0).ceil() as usize * 4096,
                device,
            );
            self.vertices.write_buffer(device, queue);
        } else {
            self.vertices
                .write_buffer_range(queue, index..self.next_index)
                .expect("couldn't write range");
        }

        // info!(
        //     "Vertex buffer capacity: {}, length: {}, empty slots: {}",
        //     self.vertices.capacity(),
        //     self.vertices.len(),
        //     self.empty_slots
        // );

        BufferIndex {
            index,
            generation: self.current_generation,
        }
    }

    /// Called at the end of a frame. Potentially triggers refragmentation.
    pub fn pass(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        if self.empty_slots > 500 && self.empty_slots > self.vertices.capacity() as u32 / 2 {
            let old_generation = self.current_generation;
            *self = Self::new(device, queue);
            self.current_generation = old_generation + 1;
        }
    }

    /// Called by an occluder to subtract it's total number of vertices from the allocated space.
    pub fn free_indices(&mut self, n_indices: u32, generation: u32) {
        if generation != self.current_generation {
            return;
        }

        self.empty_slots += n_indices;
    }
}

/// An index given and returned to the various buffer structures.
///
/// This is used for storing an entity's slot in the buffer, and
/// contains a generation to keep track of buffer refragmentations.
#[derive(Clone, Copy)]
pub struct BufferIndex {
    pub index: usize,
    pub generation: u32,
}
