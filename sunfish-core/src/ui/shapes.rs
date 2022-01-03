use std::mem;

use bytemuck::{Pod, Zeroable};
use iced_wgpu::wgpu;
use lyon::tessellation;
use lyon::tessellation::{FillVertex, StrokeVertex};
use serde::Deserialize;

// TODO: move descriptor for GpuVertex trait
use crate::ui::buffer_memory;
use crate::ui::buffer_memory::GpuVertex;

pub type Buffers = tessellation::VertexBuffers<ShapeVertex, u16>;

#[derive(Clone, Debug, Deserialize)]
pub enum Polarity {
    Unipolar,
    Bipolar,
}

// TODO: move to Coords
#[derive(Debug)]
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

    pub fn constrain_resize(&self, new_width: u32, new_height: u32, _ratio: f32) -> (u32, u32) {
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
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

impl buffer_memory::GpuVertex for ShapeVertex {
    fn descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float3,
                },
            ],
        }
    }
}

pub struct ShapeVertexBuilder<'a> {
    pub color: [f32; 3],
    pub screen_metrics: &'a ScreenMetrics,
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

pub fn create_pipeline(
    device: &wgpu::Device,
    swapchain_format: &wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shape_vs_module =
        device.create_shader_module(&wgpu::include_spirv!("shader_shape.vert.spv"));
    let shape_fs_module =
        device.create_shader_module(&wgpu::include_spirv!("shader_shape.frag.spv"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shape_render_pipeline_layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shape_render_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shape_vs_module,
            entry_point: "main",
            buffers: &[ShapeVertex::descriptor()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shape_fs_module,
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: *swapchain_format,
                alpha_blend: wgpu::BlendState::REPLACE,
                color_blend: wgpu::BlendState::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
    });
    pipeline
}

pub struct ShapesBuilder<'a> {
    pub builder: buffer_memory::GpuShapeCollectionBuilder<ShapeVertex>,
    device: &'a wgpu::Device,
    swapchain_format: &'a wgpu::TextureFormat,
}

impl<'a> ShapesBuilder<'a> {
    pub fn with_capacity(
        capacity: usize,
        device: &'a wgpu::Device,
        swapchain_format: &'a wgpu::TextureFormat,
    ) -> Self {
        ShapesBuilder {
            builder: buffer_memory::GpuShapeCollectionBuilder::with_capacity(capacity),
            device,
            swapchain_format,
        }
    }

    pub fn add(&mut self, shape: buffer_memory::GpuShape<ShapeVertex>) -> usize {
        self.builder.add(shape)
    }

    pub fn build(self) -> Shapes {
        let pipeline = create_pipeline(self.device, self.swapchain_format);
        let shapes = self.builder.build();
        let bufmem = buffer_memory::BufferMemory::new(self.device, pipeline, &shapes);
        Shapes { shapes, bufmem }
    }
}

pub struct Shapes {
    pub shapes: buffer_memory::GpuShapeCollection<ShapeVertex>,
    pub bufmem: buffer_memory::BufferMemory<ShapeVertex>,
}

impl Shapes {
    pub fn update(&mut self, index: usize, vertices: &[ShapeVertex], indices: &[u16]) {
        self.shapes.update(index, vertices, indices)
    }

    pub fn render<'a>(&'a self, rpass: wgpu::RenderPass<'a>) -> wgpu::RenderPass<'a> {
        buffer_memory::render(&self.bufmem, rpass, None)
    }
}
