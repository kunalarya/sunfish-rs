use bytemuck::{Pod, Zeroable};
use std::mem;

use crate::ui::buffers;
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
        // let x = screen_metrics.norm_x_to_corrected(self.position[0]) * 2.0 - 1.0;
        // let y = screen_metrics.norm_y_to_corrected(self.position[1]) * -2.0 + 1.0;
        let x0 = self.position[0];
        let y0 = self.position[1];
        let x1 = screen_metrics.norm_x_to_corrected(x0);
        let y1 = screen_metrics.norm_y_to_corrected(y0);
        let x = x1 * 2.0 - 1.0;
        let y = y1 * -2.0 + 1.0;
        // TODO: let x = screen_metrics.norm_x_to_corrected(x);
        // TODO: let y = screen_metrics.norm_y_to_corrected(y);
        SpriteVertex {
            position: [x, y, 0.0],
            tex_coords: self.tex_coords,
        }
    }

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

pub struct Sprite {
    pub pos: UserVec2,
    pub size: UserVec2,
    pub src_px: SpriteSource,
}

#[derive(Default)]
pub struct SpriteUpdate {
    pub pos: Option<UserVec2>,
    pub size: Option<UserVec2>,
    pub src_px: Option<SpriteSource>,
}

pub struct SpriteSheet {
    // Sprites to render from sheet.
    sprites: Vec<Sprite>,
    // TODO: Cache texture width, height as floats.
    texture: texture::Texture,
}

impl SpriteSheet {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, filename: &str) -> Self {
        let texture_bytes = std::fs::read(filename).unwrap();
        println!("Loading spritesheet...");
        let texture =
            texture::Texture::from_bytes(device, queue, &texture_bytes, filename).unwrap();
        //let _ = texture::Texture::from_png(&device, &queue, &filename, &filename);
        SpriteSheet {
            sprites: vec![],
            texture,
        }
    }

    pub fn update_sprite(&mut self, index: usize, update: &SpriteUpdate) {
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
    }
    pub fn add(&mut self, sprite: Sprite) -> usize {
        let index = self.sprites.len();
        self.sprites.push(sprite);
        index
    }
}

pub struct BoundSpriteSheet {
    // Buffer to store raw GPU vertices & indices.
    vertices: Vec<SpriteVertex>,
    indices: Vec<u16>,
    buffers: buffers::VertexBuffers<SpriteVertex>,

    sprite_render_pipeline: wgpu::RenderPipeline,
    sprite_bind_group: wgpu::BindGroup,
}

impl BoundSpriteSheet {
    pub fn new(
        device: &wgpu::Device,
        swapchain_format: &wgpu::TextureFormat,
        sheet: &SpriteSheet,
        screen_metrics: &ScreenMetrics,
    ) -> Self {
        // TODO: Rather than reallocate these each time, we can keep these vectors
        // as is, then recreate the GPU buffer.
        let mut vertices = vec![SpriteVertex::zero(); 4 * sheet.sprites.len()];
        let mut indices = vec![0; 6 * sheet.sprites.len()];

        let buffers =
            Self::create_buffers(device, &mut vertices, &mut indices, sheet, screen_metrics);

        println!("Creating sprite bind groups...");
        let sprite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let sprite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sprite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sheet.texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sheet.texture.sampler),
                },
            ],
            label: Some("sprite_bind_group"),
        });

        let sprite_vs_module =
            device.create_shader_module(wgpu::include_spirv!("shader_sprite.vert.spv"));
        let sprite_fs_module =
            device.create_shader_module(wgpu::include_spirv!("shader_sprite.frag.spv"));

        println!("Creating pipelines...");
        let sprite_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sprite_render_pipeline_layout"),
                bind_group_layouts: &[&sprite_bind_group_layout],
                push_constant_ranges: &[],
            });

        let sprite_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite_render_pipeline"),
                layout: Some(&sprite_render_pipeline_layout),
                vertex_stage: wgpu::ProgrammableStageDescriptor {
                    module: &sprite_vs_module,
                    entry_point: "main",
                },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                    module: &sprite_fs_module,
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
        Self {
            // cached/pre-allocated memory.
            vertices,
            indices,
            // GPU-mapped vertices and indices.
            buffers,
            sprite_render_pipeline,
            sprite_bind_group,
        }
    }

    fn create_buffers(
        device: &wgpu::Device,
        vertices: &mut Vec<SpriteVertex>,
        indices: &mut Vec<u16>,
        sheet: &SpriteSheet,
        screen_metrics: &ScreenMetrics,
    ) -> buffers::VertexBuffers<SpriteVertex> {
        let vertex_buf_len = 4 * sheet.sprites.len();
        vertices.resize(vertex_buf_len, SpriteVertex::zero());
        let index_buf_len = 6 * sheet.sprites.len();
        indices.resize(index_buf_len, 0);

        let (tw, th) = sheet.texture.size;
        let tw = tw as f32;
        let th = th as f32;

        let mut i = 0;
        for (sprite_i, sprite) in sheet.sprites.iter().enumerate() {
            // TODO:
            let (x, y) = sprite.pos.unpack();
            let (w, h) = sprite.size.unpack();

            let xw = x + w;
            let yh = y + h;

            // source rect
            let (src_x, src_y, src_xw, src_yh) = (
                sprite.src_px.src_rect[0] / tw,
                sprite.src_px.src_rect[1] / th,
                sprite.src_px.src_rect[2] / tw,
                sprite.src_px.src_rect[3] / th,
            );
            let vi = sprite_i * 4;
            vertices[vi] = SpriteVertex {
                //position: [x * 2.0 - 1.0, y * -2.0 + 1.0, 0.0],
                position: [x, y, 0.0],
                tex_coords: [src_x, src_y],
            }
            .correct(screen_metrics);
            vertices[vi + 1] = SpriteVertex {
                //position: [xw * 2.0 - 1.0, y * -2.0 + 1.0, 0.0],
                position: [xw, y, 0.0],
                tex_coords: [src_xw, src_y],
            }
            .correct(screen_metrics);
            vertices[vi + 2] = SpriteVertex {
                //position: [xw * 2.0 - 1.0, yh * -2.0 + 1.0, 0.0],
                position: [xw, yh, 0.0],
                tex_coords: [src_xw, src_yh],
            }
            .correct(screen_metrics);
            vertices[vi + 3] = SpriteVertex {
                // position: [x * 2.0 - 1.0, yh * -2.0 + 1.0, 0.0],
                position: [x, yh, 0.0],
                tex_coords: [src_x, src_yh],
            }
            .correct(screen_metrics);
            let ii = sprite_i * 6;
            let sprite_indices: [u16; 6] = [i, i + 1, i + 2, i + 2, i + 3, i];
            indices[ii..ii + 6].copy_from_slice(&sprite_indices);
            i += 4;
        }
        buffers::VertexBuffers::new(device, vertices, indices)
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        sheet: &SpriteSheet,
        screen_metrics: &ScreenMetrics,
    ) {
        self.buffers = Self::create_buffers(
            device,
            &mut self.vertices,
            &mut self.indices,
            sheet,
            screen_metrics,
        );
    }
}

pub fn render<'a>(
    bound_spritesheet: &'a BoundSpriteSheet,
    mut rpass: wgpu::RenderPass<'a>,
    _sheet: &SpriteSheet,
) -> wgpu::RenderPass<'a> {
    // TODO: recreate vertices

    rpass.set_pipeline(&bound_spritesheet.sprite_render_pipeline);
    rpass.set_bind_group(0, &bound_spritesheet.sprite_bind_group, &[]);
    rpass.set_vertex_buffer(0, bound_spritesheet.buffers.vertices.buf.slice(..));
    rpass.set_index_buffer(bound_spritesheet.buffers.indices.buf.slice(..));
    rpass.draw_indexed(0..bound_spritesheet.buffers.indices.size as u32, 0, 0..1);
    rpass
}
