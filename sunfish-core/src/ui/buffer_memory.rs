use std::collections::HashSet;

use lyon::tessellation;

use crate::ui::buffers;

pub type Buffers<V> = tessellation::VertexBuffers<V, u16>;

pub trait GpuVertex: bytemuck::Zeroable + bytemuck::Pod + Clone + std::fmt::Debug {
    fn descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a>;
}

/// A Shape captures the CPU side of a single polygon with a
/// max number of vertices known ahead of time (to simplify
/// GPU-side memory management).
pub struct GpuShape<V: GpuVertex> {
    vertices: Vec<V>,
    indices: Vec<u16>,
    /// Max vertices and indices allocated for this shape.
    max_v_count: usize,
    max_i_count: usize,
}

impl<V: GpuVertex> GpuShape<V> {
    pub fn new(
        vertices: Vec<V>,
        indices: Vec<u16>,
        max_v_count: usize,
        max_i_count: usize,
    ) -> Self {
        Self {
            vertices,
            indices,
            max_v_count,
            max_i_count,
        }
    }

    pub fn from_lyon(shapes: Buffers<V>, max_v_count: usize, max_i_count: usize) -> Self {
        GpuShape {
            vertices: shapes.vertices,
            indices: shapes.indices,
            max_v_count,
            max_i_count,
        }
    }

    fn update(&mut self, vertices: &[V], indices: &[u16]) {
        // TODO: Copy the slices in place.
        let vertices = if vertices.len() > self.max_v_count {
            // TODO: Should we just truncate and log?
            log::warn!("Shape::update received vertex buffer larger than max_v_count");
            &vertices[..self.max_v_count]
        } else {
            vertices
        };

        let indices = if indices.len() > self.max_i_count {
            // TODO: Should we just truncate and log?
            log::warn!("Shape::update received index buffer larger than max_i_count");
            &indices[..self.max_i_count]
        } else {
            indices
        };

        // Replace them in-memory.
        self.vertices.resize(vertices.len(), V::zeroed());
        self.vertices.clone_from_slice(vertices);

        // TODO: Maybe Shapes and BoundShapes should be merged, then we can
        // look directly at the offset from ind_ranges.
        self.indices.resize(indices.len(), 0);
        self.indices.clone_from_slice(indices);
    }
}

pub struct GpuShapeCollectionBuilder<V: GpuVertex> {
    shapes: Vec<GpuShape<V>>,
}

impl<V: GpuVertex> GpuShapeCollectionBuilder<V> {
    pub fn with_capacity(shape_count: usize) -> Self {
        Self {
            shapes: Vec::with_capacity(shape_count),
        }
    }

    pub fn add(&mut self, shape: GpuShape<V>) -> usize {
        let index = self.shapes.len();
        self.shapes.push(shape);
        index
    }

    pub fn build(self) -> GpuShapeCollection<V> {
        let shape_count = self.shapes.len();
        GpuShapeCollection {
            shapes: self.shapes,
            shapes_to_update: HashSet::with_capacity(shape_count),
        }
    }
}

pub struct GpuShapeCollection<V: GpuVertex> {
    shapes: Vec<GpuShape<V>>,
    shapes_to_update: HashSet<usize>,
}

impl<V: GpuVertex> GpuShapeCollection<V> {
    pub fn update(&mut self, index: usize, vertices: &[V], indices: &[u16]) {
        self.shapes_to_update.insert(index);
        if let Some(shape) = self.shapes.get_mut(index) {
            shape.update(vertices, indices)
        } else {
            log::warn!("Bad GpuShapeCollection index: {}", index);
        }
    }
}

struct VerRanges(Vec<std::ops::Range<u32>>);
struct IndRanges(Vec<std::ops::Range<u32>>);

pub struct BufferMemory<V: GpuVertex> {
    pub pipeline: wgpu::RenderPipeline,
    buffers: buffers::VertexBuffers<V>,
    ver_ranges: VerRanges,
    ind_ranges: IndRanges,
}

impl<V: GpuVertex> BufferMemory<V> {
    pub fn new(
        device: &wgpu::Device,
        pipeline: wgpu::RenderPipeline,
        shapes: &GpuShapeCollection<V>,
    ) -> Self {
        let (ver_ranges, ind_ranges, ver_buf, ind_buf) = pack_shapes(&shapes.shapes);
        let buffers = buffers::VertexBuffers::new(device, &ver_buf, &ind_buf);
        BufferMemory {
            pipeline,
            buffers,
            ver_ranges,
            ind_ranges,
        }
    }
}

fn pack_shapes<V: GpuVertex>(shapes: &[GpuShape<V>]) -> (VerRanges, IndRanges, Vec<V>, Vec<u16>) {
    // Compute total buffer size.
    let tot_ver_buf: usize = shapes.iter().map(|shape| shape.max_v_count).sum();
    let tot_ind_buf: usize = shapes.iter().map(|shape| shape.max_i_count).sum();

    // Round up to nearest 4. TODO: figure out how to get the 4.0
    let tot_ind_buf = (((tot_ind_buf as f32 / 4.0).ceil()) * 4.0) as usize;

    //log::info!("Shapes len={}, tot_ind_buf={}", shapes.len(), tot_ind_buf);

    let mut ver_buf = vec![V::zeroed(); tot_ver_buf];
    let mut ind_buf = vec![0u16; tot_ind_buf];

    let mut ver_offset = 0;
    let mut ver_last_offset = 0;

    let mut ind_offset = 0;
    let mut ind_last_offset = 0;

    let mut ver_ranges = Vec::with_capacity(shapes.len());
    let mut ind_ranges = Vec::with_capacity(shapes.len());

    for shape in shapes {
        let ver_size = shape.max_v_count;
        let ind_size = shape.max_i_count;

        // Copy vertex and index data; note that the sizes of
        // them will be <= max_{i,v}_count
        //
        // copy into the buffer
        ver_buf[ver_last_offset..ver_last_offset + shape.vertices.len()]
            .clone_from_slice(&shape.vertices);

        // We have to offset the indices to account for previous vertices.
        //log::info!("Shape vertices: {:?}", shape.vertices);
        //log::info!("      indices: {:?}", shape.indices);
        for (i, vertex_index) in shape.indices.iter().enumerate() {
            ind_buf[i + ind_last_offset] = vertex_index + ver_last_offset as u16;
        }

        ver_offset += ver_size;
        ind_offset += ind_size;

        ver_ranges.push(ver_last_offset as u32..ver_offset as u32);
        ind_ranges.push(ind_last_offset as u32..ind_offset as u32);

        ver_last_offset = ver_offset;
        ind_last_offset = ind_offset;
    }
    (
        VerRanges(ver_ranges),
        IndRanges(ind_ranges),
        ver_buf,
        ind_buf,
    )
}

pub fn update<V: GpuVertex>(
    device: &wgpu::Device,
    shapes: &mut GpuShapeCollection<V>,
    bufmem: &mut BufferMemory<V>,
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
) {
    // - update the ind_ranges
    for shape_index in shapes.shapes_to_update.drain() {
        let shape = &shapes.shapes[shape_index];

        let ver_offset = bufmem.ver_ranges.0[shape_index].start as u64;
        let ver_size = shape.vertices.len() as u64;

        let ind_offset = bufmem.ind_ranges.0[shape_index].start as u64;
        let ind_size = shape.indices.len() as u64;

        // TODO: These calls dig into the guts of buffers; could probably
        // benefit from a refactor.

        // Update vertices.
        let ver_elm_size = bufmem.buffers.vertices.element_size() as u64;
        let ver_buf_size = ver_elm_size * ver_size as u64;
        if ver_buf_size > 0 {
            staging_belt
                .write_buffer(
                    encoder,
                    &bufmem.buffers.vertices.buf,
                    ver_offset * ver_elm_size,
                    wgpu::BufferSize::new(ver_buf_size).unwrap(),
                    device,
                )
                .copy_from_slice(bytemuck::cast_slice(&shape.vertices));
        }
        let start = bufmem.ver_ranges.0[shape_index].start;
        bufmem.ver_ranges.0[shape_index].end = start + ver_size as u32;

        // Update indices.
        let ind_elm_size = bufmem.buffers.indices.element_size() as u64;
        let ind_buf_size = ind_elm_size * ind_size as u64;
        if ind_buf_size > 0 {
            let mut indices_offset = vec![0u16; shape.indices.len()];
            // We have to offset the indices to account for previous vertices.
            for (i, vertex_index) in shape.indices.iter().enumerate() {
                indices_offset[i] = vertex_index + ver_offset as u16;
            }

            staging_belt
                .write_buffer(
                    encoder,
                    &bufmem.buffers.indices.buf,
                    ind_offset * ind_elm_size,                    //offset
                    wgpu::BufferSize::new(ind_buf_size).unwrap(), //size
                    device,
                )
                .copy_from_slice(bytemuck::cast_slice(&indices_offset));
        }

        let start = bufmem.ind_ranges.0[shape_index].start;
        bufmem.ind_ranges.0[shape_index].end = start + ind_size as u32;
    }
}

pub fn render<'a, V: GpuVertex>(
    bufmem: &'a BufferMemory<V>,
    mut rpass: wgpu::RenderPass<'a>,
    bind_group: Option<&'a wgpu::BindGroup>,
) -> wgpu::RenderPass<'a> {
    rpass.set_pipeline(&bufmem.pipeline);

    if let Some(bind_group) = &bind_group {
        rpass.set_bind_group(0, bind_group, &[]);
    }
    rpass.set_vertex_buffer(0, bufmem.buffers.vertices.buf.slice(..));
    rpass.set_index_buffer(bufmem.buffers.indices.buf.slice(..));

    for range in &bufmem.ind_ranges.0 {
        if !range.is_empty() {
            rpass.draw_indexed(range.clone(), 0, 0..1);
        }
    }

    rpass
}
