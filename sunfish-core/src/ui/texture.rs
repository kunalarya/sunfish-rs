use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: (u32, u32),
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        println!("> loading texture from memory...");
        let t0 = std::time::Instant::now();
        let img = image::load_from_memory(bytes)?;
        let t1 = std::time::Instant::now();
        let delta = (t1 - t0).as_secs_f32();
        println!("> loaded ({:?} seconds)", delta);

        Self::from_image(device, queue, &img, Some(label))
    }

    // pub fn from_png(device: &wgpu::Device, queue: &wgpu::Queue, filename: &str, label: &str) {
    //     // -> Result<Self> {
    //     use std::fs::File;
    //     // The decoder is a build for reader and can be used to set various decoding options
    //     // via `Transformations`. The default output transformation is `Transformations::EXPAND
    //     // | Transformations::STRIP_ALPHA`.
    //     println!("> loading texture from memory (png)...");
    //     let t0 = std::time::Instant::now();
    //     let decoder = png::Decoder::new(File::open(filename).unwrap());
    //     let (info, mut reader) = decoder.read_info().unwrap();

    //     if info.bit_depth != png::BitDepth::Eight || info.color_type != png::ColorType::RGBA {
    //         // TODO: Return an error instead.
    //         panic!(
    //             "Unsupported bit depth or color type; bit depth: {:?}, color type: {:?}",
    //             info.bit_depth, info.color_type
    //         );
    //     }

    //     // Allocate the output buffer.
    //     let mut buf = vec![0; info.buffer_size()];

    //     // Read the next frame. An APNG might contain multiple frames (only support one)
    //     reader.next_frame(&mut buf).unwrap();
    //     // Inspect more details of the last read frame.
    //     //buf
    //     //Self::from_image(device, queue, &img, Some(label))
    //     let t1 = std::time::Instant::now();
    //     let delta = (t1 - t0).as_secs_f32();
    //     println!("> loaded ({:?} seconds)", delta);
    // }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.as_rgba8().unwrap();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        println!("> writing texture to queue...");
        queue.write_texture(
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            rgba,
            wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: 4 * dimensions.0,
                rows_per_image: dimensions.1,
            },
            size,
        );

        println!("> creating view and sampler...");
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            size: (dimensions.0, dimensions.1),
        })
    }
}
