//! The page pool: where converted meshes' vertices and cluster indices live on
//! the GPU.
//!
//! Three buffers share one pair of allocators: positions and packed attributes
//! by vertex, and the index words (cluster vertex lists, then triangle bytes)
//! by word. A page is one allocation in each. Meshes come and go as models
//! load and unload, so space is handed out and returned with
//! `offset_allocator`, the allocator Bevy's own mesh and skin buffers use,
//! rather than by rebuilding everything whenever the set of meshes changes.
//!
//! The allocators span far more than is ever backed. The buffers are only as
//! large as the highest allocation, grown by copying when a page lands past
//! their end; the allocator fills from the start and reuses holes first, so
//! that stays close to what is in use.

use bevy::render::render_resource::{Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use offset_allocator::{Allocation, Allocator};

/// Bytes per vertex in each vertex buffer: three floats of position, and the
/// four words `scene::pack_attributes` writes.
const POSITION_BYTES: u64 = 12;
const ATTRIBUTE_BYTES: u64 = 16;

/// The allocators' span, far past any real scene: they hand out offsets, and
/// only the part in use is backed by a buffer.
const SPAN: u32 = 1 << 30;
const MAX_ALLOCATIONS: u32 = 1 << 20;

/// Where a page landed.
#[derive(Clone, Copy)]
pub(crate) struct Page {
    pub vertices: Allocation,
    pub words: Allocation,
    pub vertex_count: u32,
    pub word_count: u32,
}

pub(crate) struct Pool {
    vertices: Allocator,
    words: Allocator,
    /// How many vertices and words the buffers hold.
    vertex_capacity: u32,
    word_capacity: u32,
    pub positions: Buffer,
    pub attributes: Buffer,
    pub indices: Buffer,
    /// Vertices and words in live pages, for reporting.
    pub vertices_used: u64,
    pub words_used: u64,
}

impl Pool {
    pub fn new(device: &RenderDevice) -> Self {
        let (vertex_capacity, word_capacity) = (1 << 16, 1 << 18);
        Self {
            vertices: Allocator::with_max_allocs(SPAN, MAX_ALLOCATIONS),
            words: Allocator::with_max_allocs(SPAN, MAX_ALLOCATIONS),
            vertex_capacity,
            word_capacity,
            positions: buffer(device, "vg_positions", vertex_capacity as u64 * POSITION_BYTES),
            attributes: buffer(device, "vg_attributes", vertex_capacity as u64 * ATTRIBUTE_BYTES),
            indices: buffer(device, "vg_indices", word_capacity as u64 * 4),
            vertices_used: 0,
            words_used: 0,
        }
    }

    /// Bytes the live pages take.
    pub fn bytes_used(&self) -> u64 {
        self.vertices_used * (POSITION_BYTES + ATTRIBUTE_BYTES) + self.words_used * 4
    }

    /// Bytes the buffers take, used or not.
    pub fn bytes_allocated(&self) -> u64 {
        self.positions.size() + self.attributes.size() + self.indices.size()
    }

    /// Room for a page of `vertex_count` vertices and `word_count` index
    /// words, growing the buffers when it lands past their end. Returns the
    /// page, or `None` only when the allocators themselves are exhausted.
    /// When the buffers were replaced, bind groups must follow; the caller
    /// bumps its generation on `true`.
    pub fn allocate(
        &mut self,
        device: &RenderDevice,
        queue: &RenderQueue,
        vertex_count: u32,
        word_count: u32,
    ) -> Option<(Page, bool)> {
        // Never zero: a page always has somewhere to point.
        let vertices = self.vertices.allocate(vertex_count.max(1))?;
        let Some(words) = self.words.allocate(word_count.max(1)) else {
            self.vertices.free(vertices);
            return None;
        };
        let grown = self.reserve(device, queue, vertices.offset + vertex_count, words.offset + word_count);
        self.vertices_used += u64::from(vertex_count);
        self.words_used += u64::from(word_count);
        Some((
            Page {
                vertices,
                words,
                vertex_count,
                word_count,
            },
            grown,
        ))
    }

    pub fn free(&mut self, page: Page) {
        self.vertices.free(page.vertices);
        self.words.free(page.words);
        self.vertices_used -= u64::from(page.vertex_count);
        self.words_used -= u64::from(page.word_count);
    }

    /// Grow the buffers once to fit this many more vertices and words than are
    /// in use, ahead of a batch of uploads.
    ///
    /// A model arriving uploads every skinned mesh whole and every root group
    /// in one frame. Left to `allocate`, the pool grew by half for each one
    /// that did not fit, a dozen times in a row from its starting size, and
    /// every set of buffers it replaced stayed alive until the GPU had
    /// finished with it: a few seconds of several times the pool's final size,
    /// right as the model appeared. Returns whether the buffers were replaced.
    pub fn reserve_more(&mut self, device: &RenderDevice, queue: &RenderQueue, vertices: u64, words: u64) -> bool {
        let clamp = |n: u64| n.min(u64::from(SPAN)) as u32;
        let vertices = clamp(self.vertices_used + vertices);
        let words = clamp(self.words_used + words);
        self.reserve(device, queue, vertices, words)
    }

    /// Make the buffers hold at least this many vertices and words. Copies the
    /// old contents into the new buffers and submits that at once, so writes
    /// queued afterwards (which wgpu applies before the next submission's
    /// commands) land in the new buffers on top of the copy, never under it.
    fn reserve(&mut self, device: &RenderDevice, queue: &RenderQueue, vertices: u32, words: u32) -> bool {
        if vertices <= self.vertex_capacity && words <= self.word_capacity {
            return false;
        }
        let grow = |have: u32, need: u32| if need <= have { have } else { need.max(have + have / 2) };
        let (vertex_capacity, word_capacity) = (grow(self.vertex_capacity, vertices), grow(self.word_capacity, words));
        let positions = buffer(device, "vg_positions", vertex_capacity as u64 * POSITION_BYTES);
        let attributes = buffer(device, "vg_attributes", vertex_capacity as u64 * ATTRIBUTE_BYTES);
        let indices = buffer(device, "vg_indices", word_capacity as u64 * 4);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("vg_pool_grow"),
        });
        encoder.copy_buffer_to_buffer(&self.positions, 0, &positions, 0, self.positions.size());
        encoder.copy_buffer_to_buffer(&self.attributes, 0, &attributes, 0, self.attributes.size());
        encoder.copy_buffer_to_buffer(&self.indices, 0, &indices, 0, self.indices.size());
        queue.submit([encoder.finish()]);
        (self.positions, self.attributes, self.indices) = (positions, attributes, indices);
        (self.vertex_capacity, self.word_capacity) = (vertex_capacity, word_capacity);
        true
    }
}

fn buffer(device: &RenderDevice, label: &'static str, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}
