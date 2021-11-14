use std::collections::HashSet;
use std::mem;

use bytemuck::{Pod, Zeroable};
use euclid::vec2;
use lyon::geom::euclid;
use lyon::geom::euclid::Box2D;
use lyon::math::{point, Point};
use lyon::math::{Angle, Vector};
use lyon::path::builder::Build;
use lyon::path::Path;
use lyon::path::Polygon;
use lyon::tessellation;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, LineJoin, StrokeOptions, StrokeTessellator,
};
use lyon::tessellation::{FillVertex, StrokeVertex};
use serde::Deserialize;

use crate::ui::buffers;
use crate::ui::coords::Rect;

pub type Buffers = tessellation::VertexBuffers<ShapeVertex, u16>;

#[derive(Clone, Debug, Deserialize)]
pub enum Polarity {
    Unipolar,
    Bipolar,
}

// TODO: move to Coords
pub struct ScreenMetrics {
    pub width_f32: f32,
    pub height_f32: f32,
    pub width_u32: u32,
    pub height_u32: u32,
    pub scale_factor: f64,
    pub ratio: f32,
}

impl ScreenMetrics {
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Self {
        let width_f32 = width as f32;
        let height_f32 = height as f32;
        let ratio = width_f32 / height_f32;
        log::info!(
            "width={} height={} width_f32={} height_f32={} ratio={:?}",
            width,
            height,
            width_f32,
            height_f32,
            ratio
        );
        ScreenMetrics {
            width_f32,
            height_f32,
            width_u32: width,
            height_u32: height,
            scale_factor,
            ratio,
        }
    }

    pub fn norm_x_to_corrected(&self, x: f32) -> f32 {
        x
    }

    pub fn norm_y_to_corrected(&self, y: f32) -> f32 {
        y * self.ratio
    }

    pub fn corrected_x_to_norm(&self, x: f32) -> f32 {
        x
    }

    pub fn corrected_y_to_norm(&self, y: f32) -> f32 {
        y / self.ratio
    }

    pub fn screen_x_to_norm(&self, x: f32) -> f32 {
        self.corrected_x_to_norm(x / self.width_f32)
    }

    pub fn screen_y_to_norm(&self, y: f32) -> f32 {
        self.corrected_y_to_norm(y / self.height_f32)
    }

    pub fn norm_x_to_screen(&self, x: f32) -> f32 {
        self.norm_x_to_corrected(x) * self.width_f32
    }

    pub fn norm_y_to_screen(&self, y: f32) -> f32 {
        self.norm_y_to_corrected(y) * self.height_f32
    }

    pub fn constrain_resize(&self, new_width: u32, new_height: u32, ratio: f32) -> (u32, u32) {
        let new_width_f32 = new_width as f32;
        let new_height_f32 = new_height as f32;
        let new_ratio = new_width_f32 / new_height_f32;

        let same_width = new_width == self.width_u32;
        let same_height = new_height == self.height_u32;

        log::info!(
            "width: {}, height: {} (ratio: {}) -> new_width: {}, new_height: {} (ratio: {}); same_width={}, same_height={}",
            self.width_u32,
            self.height_u32,
            self.ratio,
            new_width,
            new_height,
            new_ratio,
            same_width,
            same_height
        );

        // If the new ratio is greater, then we've expanded
        // our width farther; so we'll bump up the height to
        // preserve the ratio.
        // ratio = w / h
        // thus h = w / ratio
        let correct_height = !same_width || new_ratio > self.ratio;
        // Likewise if the new ratio is smaller, the height
        // has become longer, so we'll increase width.
        // ratio = w / h
        // thus w = h * ratio
        let correct_width = !same_height || new_ratio < self.ratio;
        if correct_height {
            let corrected_height = (new_width_f32 / self.ratio).round() as u32;
            let (w, h) = (new_width, corrected_height);
            log::info!(
                "Corrected height: {}, new ratio: {}",
                corrected_height,
                w as f32 / h as f32
            );
            (w, h)
        } else {
            let corrected_width = (new_height_f32 * self.ratio).round() as u32;
            let (w, h) = (corrected_width, new_height);
            log::info!(
                "Corrected width: {}, new ratio: {}",
                corrected_width,
                w as f32 / h as f32
            );
            (w, h)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn to_array3(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
    pub fn to_array4(&self) -> [f32; 4] {
        [self.r, self.g, self.b, 1.0]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShapeVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl ShapeVertex {
    pub fn descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttributeDescriptor {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float3,
                },
            ],
        }
    }
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

pub struct ShapeVertexBuilder<'a> {
    pub color: [f32; 3],
    screen_metrics: &'a ScreenMetrics,
}

impl<'a> lyon::tessellation::StrokeVertexConstructor<ShapeVertex> for ShapeVertexBuilder<'a> {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> ShapeVertex {
        let position = vertex.position();
        let x = self.screen_metrics.norm_x_to_corrected(position.x) * 2.0 - 1.0;
        let y = self.screen_metrics.norm_y_to_corrected(position.y) * -2.0 + 1.0;
        ShapeVertex {
            position: [x, y, 0.0],
            color: self.color,
        }
    }
}

impl<'a> lyon::tessellation::FillVertexConstructor<ShapeVertex> for ShapeVertexBuilder<'a> {
    fn new_vertex(&mut self, vertex: FillVertex) -> ShapeVertex {
        let position = vertex.position();
        let x = self.screen_metrics.norm_x_to_corrected(position.x) * 2.0 - 1.0;
        let y = self.screen_metrics.norm_y_to_corrected(position.y) * -2.0 + 1.0;
        ShapeVertex {
            position: [x, y, 0.0],
            color: self.color,
        }
    }
}

/// A Shape captures the CPU side of a single polygon with a
/// max number of vertices known ahead of time (to simplify
/// GPU-side memory management).
pub struct Shape {
    vertices: Vec<ShapeVertex>,
    indices: Vec<u16>,
    /// Max vertices and indices allocated for this shape.
    max_v_count: usize,
    max_i_count: usize,
}

impl Shape {
    pub fn new(
        vertices: Vec<ShapeVertex>,
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

    pub fn from_lyon(shapes: Buffers, max_v_count: usize, max_i_count: usize) -> Self {
        Shape {
            vertices: shapes.vertices,
            indices: shapes.indices,
            max_v_count,
            max_i_count,
        }
    }

    fn update(&mut self, vertices: &Vec<ShapeVertex>, indices: &Vec<u16>) {
        // TODO: Copy the slices in place.
        let vertices = if vertices.len() > self.max_v_count {
            // TODO: Should we just truncate and log?
            log::warn!("Shape::update received vertex buffer larger than max_v_count");
            &vertices[..self.max_v_count]
        } else {
            &vertices[..]
        };

        let indices = if indices.len() > self.max_i_count {
            // TODO: Should we just truncate and log?
            log::warn!("Shape::update received index buffer larger than max_i_count");
            &indices[..self.max_i_count]
        } else {
            &indices[..]
        };

        // Replace them in-memory.
        self.vertices.resize(vertices.len(), ShapeVertex::zeroed());
        self.vertices.clone_from_slice(vertices);

        // TODO: Maybe Shapes and BoundShapes should be merged, then we can
        // look directly at the offset from ind_ranges.
        self.indices.resize(indices.len(), 0);
        self.indices.clone_from_slice(indices);
    }
}

pub struct Shapes {
    shapes: Vec<Shape>,
    shapes_to_update: HashSet<usize>,
}

impl Shapes {
    pub fn with_capacity(shape_count: usize) -> Self {
        Self {
            shapes: Vec::with_capacity(shape_count),
            shapes_to_update: HashSet::with_capacity(shape_count),
        }
    }

    pub fn add(&mut self, shape: Shape) -> usize {
        let index = self.shapes.len();
        self.shapes.push(shape);
        index
    }

    pub fn update(&mut self, index: usize, vertices: &Vec<ShapeVertex>, indices: &Vec<u16>) {
        self.shapes_to_update.insert(index);
        self.shapes
            .get_mut(index)
            .map(|shape| shape.update(vertices, indices));
    }
}

struct VerRanges(Vec<std::ops::Range<u32>>);
struct IndRanges(Vec<std::ops::Range<u32>>);

pub struct BoundShapes {
    pub shape_render_pipeline: wgpu::RenderPipeline,
    buffers: buffers::VertexBuffers<ShapeVertex>,
    ver_ranges: VerRanges,
    ind_ranges: IndRanges,
    shadow_vertices: Vec<ShapeVertex>,
    shadow_indices: Vec<u16>,
}

impl BoundShapes {
    pub fn new(
        device: &wgpu::Device,
        swapchain_format: &wgpu::TextureFormat,
        shapes: &Shapes,
    ) -> Self {
        let (ver_ranges, ind_ranges, ver_buf, ind_buf) = Self::pack_shapes(shapes);
        let buffers = buffers::VertexBuffers::new(&device, &ver_buf, &ind_buf);

        let shape_vs_module =
            device.create_shader_module(wgpu::include_spirv!("shader_shape.vert.spv"));
        let shape_fs_module =
            device.create_shader_module(wgpu::include_spirv!("shader_shape.frag.spv"));

        let shape_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shape_render_pipeline_layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let shape_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shape_render_pipeline"),
                layout: Some(&shape_render_pipeline_layout),
                vertex_stage: wgpu::ProgrammableStageDescriptor {
                    module: &shape_vs_module,
                    entry_point: "main",
                },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                    module: &shape_fs_module,
                    entry_point: "main",
                }),
                // Use the default rasterizer state: no culling, no depth bias
                rasterization_state: None,
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[(*swapchain_format).into()],
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[ShapeVertex::descriptor()],
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false,
            });
        BoundShapes {
            shape_render_pipeline,
            buffers,
            ver_ranges,
            ind_ranges,
            shadow_vertices: ver_buf,
            shadow_indices: ind_buf,
        }
    }

    fn pack_shapes(shapes: &Shapes) -> (VerRanges, IndRanges, Vec<ShapeVertex>, Vec<u16>) {
        // Compute total buffer size.
        let tot_ver_buf: usize = shapes.shapes.iter().map(|shape| shape.max_v_count).sum();
        let tot_ind_buf: usize = shapes.shapes.iter().map(|shape| shape.max_i_count).sum();

        // Round up to nearest 4. TODO: figure out how to get the 4.0
        let tot_ind_buf = (((tot_ind_buf as f32 / 4.0).ceil()) * 4.0) as usize;

        let mut ver_buf = vec![ShapeVertex::zeroed(); tot_ver_buf];
        let mut ind_buf = vec![0u16; tot_ind_buf];

        let mut ver_offset = 0;
        let mut ver_last_offset = 0;

        let mut ind_offset = 0;
        let mut ind_last_offset = 0;

        let mut ver_ranges = Vec::with_capacity(shapes.shapes.len());
        let mut ind_ranges = Vec::with_capacity(shapes.shapes.len());

        for shape in &shapes.shapes {
            let ver_size = shape.max_v_count;
            let ind_size = shape.max_i_count;

            // Copy vertex and index data; note that the sizes of
            // them will be <= max_{i,v}_count
            //
            // copy into the buffer
            ver_buf[ver_last_offset..ver_last_offset + shape.vertices.len()]
                .clone_from_slice(&shape.vertices);

            // We have to offset the indices to account for previous vertices.
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

    fn update_shadow_buffer(&mut self, shapes: &mut Shapes) {
        // TODO:
        // - update the ind_ranges
        for shape_index in shapes.shapes_to_update.drain() {
            let shape = &shapes.shapes[shape_index];
            let ver_offset = self.ver_ranges.0[shape_index].start;
            let ver_offset_usize = ver_offset as usize;
            let ver_size = shape.vertices.len();

            self.shadow_vertices[ver_offset_usize..ver_offset_usize + ver_size]
                .copy_from_slice(&shape.vertices);

            let start = self.ver_ranges.0[shape_index].start;
            self.ver_ranges.0[shape_index].end = start + ver_size as u32;

            // =======
            // Indices:
            // =======
            let ind_offset = self.ind_ranges.0[shape_index].start as usize;
            let ind_size = shape.indices.len();

            // We have to offset the indices to account for previous vertices.
            for (i, vertex_index) in shape.indices.iter().enumerate() {
                self.shadow_indices[i + ind_offset] = vertex_index + ver_offset as u16;
            }

            let start = self.ind_ranges.0[shape_index].start;
            self.ind_ranges.0[shape_index].end = start + ind_size as u32;
        }
    }
}

pub fn render<'a>(
    bound_shapes: &'a BoundShapes,
    mut rpass: wgpu::RenderPass<'a>,
) -> wgpu::RenderPass<'a> {
    rpass.set_pipeline(&bound_shapes.shape_render_pipeline);

    rpass.set_vertex_buffer(0, bound_shapes.buffers.vertices.buf.slice(..));
    rpass.set_index_buffer(bound_shapes.buffers.indices.buf.slice(..));

    for range in &bound_shapes.ind_ranges.0 {
        if range.len() > 0 {
            rpass.draw_indexed(range.clone(), 0, 0..1);
        }
    }

    rpass
}

#[derive(Debug)]
pub struct Arc {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub amount: f32,
    pub min_angle: f32,
    pub max_angle: f32,
    pub color: Color,
    pub stroke_width: f32,
    pub polarity: Polarity,
}

impl Arc {
    pub fn render(&self, screen_metrics: &ScreenMetrics) -> Buffers {
        let (x, y) = (self.x, self.y);

        // Build a Path.
        let mut builder = Path::svg_builder();
        let center: Point = point(x, y);

        let actual_radius = self.radius;
        let angle_range = self.max_angle - self.min_angle;
        let target_angle = self.amount * angle_range;

        let (start_angle, sweep_angle) = match &self.polarity {
            Polarity::Unipolar => (self.min_angle, target_angle),
            Polarity::Bipolar => {
                let midway = angle_range / 2.0;
                if target_angle < midway {
                    (target_angle + self.min_angle, midway - target_angle)
                } else {
                    (midway + self.min_angle, target_angle - midway)
                }
            }
        };

        let arc = lyon::geom::Arc {
            start_angle: -Angle::degrees(start_angle),
            center,
            radii: Vector::new(actual_radius, actual_radius),
            sweep_angle: -Angle::degrees(sweep_angle),
            x_rotation: Angle::degrees(180.0),
        };

        // If the current position is not on the arc, move or line to the beginning of the
        // arc.
        let arc_start = arc.from();
        builder.move_to(arc_start);

        arc.for_each_quadratic_bezier(&mut |curve| {
            builder.quadratic_bezier_to(curve.ctrl, curve.to);
        });

        let path = builder.build();
        // Create the destination vertex and index buffers.
        let mut buffers: Buffers = tessellation::VertexBuffers::new();

        {
            // Create the destination vertex and index buffers.
            let mut vertex_builder = BuffersBuilder::new(
                &mut buffers,
                ShapeVertexBuilder {
                    color: self.color.to_array3(),
                    screen_metrics,
                },
            );

            // Create the tessellator.
            let mut tessellator = StrokeTessellator::new();

            // Compute the tessellation.
            let opts = StrokeOptions::default()
                .with_tolerance(0.00005)
                .with_line_width(self.stroke_width)
                .with_line_join(LineJoin::MiterClip);
            tessellator
                .tessellate(&path, &opts, &mut vertex_builder)
                .unwrap();
        }

        buffers
    }
}

pub fn update(
    device: &wgpu::Device,
    shapes: &mut Shapes,
    bound_shapes: &mut BoundShapes,
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
) {
    _delta_update(device, shapes, bound_shapes, staging_belt, encoder);
}

pub fn _update_whole_copy(
    device: &wgpu::Device,
    shapes: &mut Shapes,
    bound_shapes: &mut BoundShapes,
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
) {
    // We have to update the entire buffer each time.
    //
    // So, to minimize copies, the bound shapes object keeps shadow vectors
    // of the vertices and indices.
    bound_shapes.update_shadow_buffer(shapes);

    // let (ver_ranges, ind_ranges, ver_buf, ind_buf) = BoundShapes::pack_shapes(shapes);
    // bound_shapes.ver_ranges = ver_ranges;
    // bound_shapes.ind_ranges = ind_ranges;

    // Now copy the whole chunk
    let ver_elm_size = bound_shapes.buffers.vertices.element_size() as u64;
    let ver_buf_size = bound_shapes.buffers.vertices.size as u64 * ver_elm_size;
    staging_belt
        .write_buffer(
            encoder,
            &bound_shapes.buffers.vertices.buf,
            0,
            wgpu::BufferSize::new(ver_buf_size).unwrap(), //size
            device,
        )
        .copy_from_slice(bytemuck::cast_slice(&bound_shapes.shadow_vertices));

    // Indices
    let ind_elm_size = bound_shapes.buffers.indices.element_size() as u64;
    let ind_buf_size = bound_shapes.buffers.indices.size as u64 * ind_elm_size;
    staging_belt
        .write_buffer(
            encoder,
            &bound_shapes.buffers.indices.buf,
            0,
            wgpu::BufferSize::new(ind_buf_size).unwrap(), //size
            device,
        )
        .copy_from_slice(bytemuck::cast_slice(&bound_shapes.shadow_indices));
}

pub fn _delta_update(
    device: &wgpu::Device,
    shapes: &mut Shapes,
    bound_shapes: &mut BoundShapes,
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
) {
    // TODO:
    // - update the ind_ranges
    for shape_index in shapes.shapes_to_update.drain() {
        let shape = &shapes.shapes[shape_index];

        let ver_offset = bound_shapes.ver_ranges.0[shape_index].start as u64;
        let ver_size = shape.vertices.len() as u64;

        let ind_offset = bound_shapes.ind_ranges.0[shape_index].start as u64;
        let ind_size = shape.indices.len() as u64;

        // TODO: These calls dig into the guts of buffers; could probably
        // benefit from a refactor.

        // Update vertices.
        let ver_elm_size = bound_shapes.buffers.vertices.element_size() as u64;
        let ver_buf_size = ver_elm_size * ver_size as u64;
        if ver_buf_size > 0 {
            staging_belt
                .write_buffer(
                    encoder,
                    &bound_shapes.buffers.vertices.buf,
                    ver_offset * ver_elm_size,
                    wgpu::BufferSize::new(ver_buf_size).unwrap(),
                    device,
                )
                .copy_from_slice(bytemuck::cast_slice(&shape.vertices));
        }
        let start = bound_shapes.ver_ranges.0[shape_index].start;
        bound_shapes.ver_ranges.0[shape_index].end = start + ver_size as u32;

        // Update indices.
        let ind_elm_size = bound_shapes.buffers.indices.element_size() as u64;
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
                    &bound_shapes.buffers.indices.buf,
                    ind_offset * ind_elm_size,                    //offset
                    wgpu::BufferSize::new(ind_buf_size).unwrap(), //size
                    device,
                )
                .copy_from_slice(bytemuck::cast_slice(&indices_offset));
        }

        let start = bound_shapes.ind_ranges.0[shape_index].start;
        bound_shapes.ind_ranges.0[shape_index].end = start + ind_size as u32;
    }
}

pub fn rectangle_outline(
    rect: &Rect,
    screen_metrics: &ScreenMetrics,
    stroke_width: f32,
    color: &[f32; 3],
) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: color.clone(),
            screen_metrics,
        },
    );
    stroke_tess
        .tessellate_polygon(
            Polygon {
                points: &[
                    point(rect.x1(), rect.y1()),
                    point(rect.x2(), rect.y1()),
                    point(rect.x2(), rect.y2()),
                    point(rect.x1(), rect.y2()),
                    point(rect.x1(), rect.y1()),
                ],
                closed: true,
            },
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn rectangle_solid(rect: &Rect, screen_metrics: &ScreenMetrics) -> Buffers {
    let box2d = Box2D::new(
        euclid::point2(rect.x1(), rect.y1()),
        euclid::point2(rect.x2(), rect.y2()),
    );

    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut fill_tess = FillTessellator::new();

    let opts = FillOptions::default();
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: [1.0, 1.0, 1.0],
            screen_metrics,
        },
    );
    use lyon::tessellation::geometry_builder::*;
    fill_tess
        .tessellate_rectangle(&box2d.to_rect(), &opts, &mut vertex_builder)
        .unwrap();
    buffers
}

pub fn line_segment(
    from: &(f32, f32),
    to: &(f32, f32),
    screen_metrics: &ScreenMetrics,
    stroke_width: f32,
    color: &[f32; 3],
) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: color.clone(),
            screen_metrics,
        },
    );
    stroke_tess
        .tessellate_polygon(
            Polygon {
                points: &[point(from.0, from.1), point(to.0, to.1)],
                closed: false,
            },
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn ellipse_outline(rect: &Rect, screen_metrics: &ScreenMetrics, stroke_width: f32) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: [1.0, 1.0, 1.0],
            screen_metrics,
        },
    );
    let center: Point = point(rect.mid_x(), rect.mid_y());
    let radii = vec2(rect.width() / 2.0, rect.height() / 2.0);
    stroke_tess
        .tessellate_ellipse(
            center,
            radii,
            Angle::degrees(0.0), // x_rotation
            lyon::path::Winding::Positive,
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn circle_outline(rect: &Rect, screen_metrics: &ScreenMetrics, stroke_width: f32) -> Buffers {
    let size = if rect.width() > rect.height() {
        rect.height()
    } else {
        rect.width()
    };
    let square = Rect::centered_at(rect.mid_x(), rect.mid_y(), size, size);
    ellipse_outline(&square, screen_metrics, stroke_width)
}
