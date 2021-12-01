use bytemuck::{Pod, Zeroable};
use std::mem;

use crate::ui::buffer_memory::{
    self, BufferMemory, GpuShape, GpuShapeCollection, GpuShapeCollectionBuilder, GpuVertex,
};
use crate::ui::coords::UserVec2;
use crate::ui::shapes::ScreenMetrics;
use crate::ui::texture;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl SpriteVertex {
    pub fn zero() -> Self {
        SpriteVertex {
            position: [0.0, 0.0, 0.0],
            tex_coords: [0.0, 0.0],
        }
    }

    /// Create a new vertex corrected to screen metrics.
    pub fn correct(&self, screen_metrics: &ScreenMetrics) -> Self {
        let x0 = self.position[0];
        let y0 = self.position[1];
        let x = screen_metrics.norm_x_to_corrected(x0) * 2.0 - 1.0;
        let y = screen_metrics.norm_y_to_corrected(y0) * -2.0 + 1.0;
        SpriteVertex {
            position: [x, y, 0.0],
            tex_coords: self.tex_coords,
        }
    }
}

impl GpuVertex for SpriteVertex {
    fn descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
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
                    format: wgpu::VertexFormat::Float2,
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpriteSource {
    pub src_rect: [f32; 4],
}

pub struct SpriteBuilder {
    pub pos: UserVec2,
    pub size: UserVec2,
    pub src_px: SpriteSource,
}

impl SpriteBuilder {
    fn build(self, shape_index: usize) -> Sprite {
        Sprite {
            pos: self.pos,
            size: self.size,
            src_px: self.src_px,
            shape_index,
        }
    }
}

fn sprite_to_vertices_and_indices(
    pos: &UserVec2,
    size: &UserVec2,
    src_px: &SpriteSource,
    screen_metrics: &ScreenMetrics,
    texture_width: f32,
    texture_height: f32,
) -> (Vec<SpriteVertex>, Vec<u16>) {
    let (x, y) = pos.unpack();
    let (w, h) = size.unpack();

    let xw = x + w;
    let yh = y + h;

    // source rect
    let (src_x, src_y, src_xw, src_yh) = (
        src_px.src_rect[0] / texture_width,
        src_px.src_rect[1] / texture_height,
        src_px.src_rect[2] / texture_width,
        src_px.src_rect[3] / texture_height,
    );
    let vertices = vec![
        SpriteVertex {
            position: [x, y, 0.0],
            tex_coords: [src_x, src_y],
        }
        .correct(screen_metrics),
        SpriteVertex {
            position: [xw, y, 0.0],
            tex_coords: [src_xw, src_y],
        }
        .correct(screen_metrics),
        SpriteVertex {
            position: [xw, yh, 0.0],
            tex_coords: [src_xw, src_yh],
        }
        .correct(screen_metrics),
        SpriteVertex {
            position: [x, yh, 0.0],
            tex_coords: [src_x, src_yh],
        }
        .correct(screen_metrics),
    ];
    let indices = vec![0, 1, 2, 2, 3, 0];
    (vertices, indices)
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub pos: UserVec2,
    pub size: UserVec2,
    pub src_px: SpriteSource,
    shape_index: usize,
}

#[derive(Default)]
pub struct SpriteUpdate {
    pub pos: Option<UserVec2>,
    pub size: Option<UserVec2>,
    pub src_px: Option<SpriteSource>,
}

pub struct SpriteSheetBuilder<'a> {
    sprites: Vec<SpriteBuilder>,
    device: &'a wgpu::Device,
    swapchain_format: &'a wgpu::TextureFormat,
    queue: &'a wgpu::Queue,
    filename: &'a str,
}

impl<'a> SpriteSheetBuilder<'a> {
    pub fn new(
        device: &'a wgpu::Device,
        swapchain_format: &'a wgpu::TextureFormat,
        queue: &'a wgpu::Queue,
        filename: &'a str,
    ) -> Self {
        Self {
            sprites: Vec::new(),
            device,
            swapchain_format,
            queue,
            filename,
        }
    }
    pub fn add(&mut self, sprite: SpriteBuilder) -> usize {
        let index = self.sprites.len();
        self.sprites.push(sprite);
        index
    }

    pub fn build(mut self, screen_metrics: &ScreenMetrics) -> SpriteSheet {
        let texture_bytes = std::fs::read(self.filename).unwrap();
        log::info!("Loading spritesheet...");
        let texture =
            texture::Texture::from_bytes(self.device, self.queue, &texture_bytes, self.filename)
                .unwrap();
        let (pipeline, bind_group) =
            create_pipeline_and_bind_group(self.device, self.swapchain_format, &texture);

        let mut gpu_shape_builder = GpuShapeCollectionBuilder::with_capacity(self.sprites.len());
        let mut sprites = Vec::with_capacity(self.sprites.len());
        let (texture_width, texture_height) = texture.size;
        for sprite_builder in self.sprites.drain(..) {
            // Create the shape vertices and index.
            let (vertices, indices) = sprite_to_vertices_and_indices(
                &sprite_builder.pos,
                &sprite_builder.size,
                &sprite_builder.src_px,
                screen_metrics,
                texture_width as f32,
                texture_height as f32,
            );
            let max_v_count = vertices.len();
            let max_i_count = indices.len();

            // Add it to the GPU shape collection.
            let shape_index =
                gpu_shape_builder.add(GpuShape::new(vertices, indices, max_v_count, max_i_count));

            // Create the shape itself.
            sprites.push(sprite_builder.build(shape_index));
        }

        // After we've added all sprite shapes, build the final shape manager.
        let shapes = gpu_shape_builder.build();

        let bufmem = BufferMemory::new(self.device, pipeline, &shapes);
        SpriteSheet {
            sprites,
            texture,
            shapes,
            bufmem,
            bind_group,
        }
    }
}

/// A GPU-mapped sprite sheet along with shapes representing different sprites that will be
/// rendered on screen.
pub struct SpriteSheet {
    // Sprites to render from sheet.
    sprites: Vec<Sprite>,

    // TODO: Cache texture width, height as floats.
    texture: texture::Texture,
    pub shapes: GpuShapeCollection<SpriteVertex>,

    pub bufmem: BufferMemory<SpriteVertex>,
    // TODO: move this into BufferMemory?
    bind_group: wgpu::BindGroup,
}

impl SpriteSheet {
    pub fn update_sprite(
        &mut self,
        index: usize,
        update: &SpriteUpdate,
        screen_metrics: &ScreenMetrics,
    ) {
        let sprite = &mut self.sprites[index];
        if let Some(pos) = &update.pos {
            sprite.pos = pos.clone();
        }
        if let Some(size) = &update.size {
            sprite.size = size.clone();
        }
        if let Some(src_px) = &update.src_px {
            sprite.src_px = src_px.clone();
        }
        // TODO: Update the buffer.
        if let Some(sprite) = self.sprites.get(index) {
            let (texture_width, texture_height) = &self.texture.size;
            let (vertices, indices) = sprite_to_vertices_and_indices(
                &sprite.pos,
                &sprite.size,
                &sprite.src_px,
                screen_metrics,
                *texture_width as f32,
                *texture_height as f32,
            );
            self.shapes.update(sprite.shape_index, &vertices, &indices);
        } else {
            log::warn!("Bad sprite index: {}", index);
        }
    }

    pub fn render<'a>(&'a self, rpass: wgpu::RenderPass<'a>) -> wgpu::RenderPass<'a> {
        buffer_memory::render(&self.bufmem, rpass, Some(&self.bind_group))
    }
}

pub fn create_pipeline_and_bind_group(
    device: &wgpu::Device,
    swapchain_format: &wgpu::TextureFormat,
    texture: &texture::Texture,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
    let vs_module = device.create_shader_module(wgpu::include_spirv!("shader_sprite.vert.spv"));
    let fs_module = device.create_shader_module(wgpu::include_spirv!("shader_sprite.frag.spv"));

    log::info!("Creating sprite bind groups...");
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    component_type: wgpu::TextureComponentType::Float,
                    dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler { comparison: false },
                count: None,
            },
        ],
        label: Some("sprite_bind_group_layout"),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&texture.sampler),
            },
        ],
        label: Some("sprite_bind_group"),
    });

    log::info!("Creating pipelines...");
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("render_pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render_pipeline"),
        layout: Some(&pipeline_layout),
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        // Use the default rasterizer state: no culling, no depth bias
        rasterization_state: None,
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[(*swapchain_format).into()],
        depth_stencil_state: None,
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[SpriteVertex::descriptor()],
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    (pipeline, bind_group)
}
