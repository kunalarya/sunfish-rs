use bytemuck::Pod;

// From iced_wgpu
#[derive(Debug)]
pub struct Buffer<T> {
    pub buf: wgpu::Buffer,
    pub size: usize,
    usage: wgpu::BufferUsage,
    _type: std::marker::PhantomData<T>,
}

impl<T> Buffer<T> {
    pub fn new(device: &wgpu::Device, size: usize, usage: wgpu::BufferUsage) -> Self {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * size) as u64,
            usage,
            mapped_at_creation: true, // TODO: What does this do?
        });
        Buffer {
            buf,
            size,
            usage,
            _type: std::marker::PhantomData,
        }
    }

    #[allow(dead_code)]
    pub fn ensure_capacity(&mut self, device: &wgpu::Device, size: usize) {
        if self.size < size {
            self.buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (std::mem::size_of::<T>() * size) as u64,
                usage: self.usage,
                mapped_at_creation: true, // TODO: What does this do?
            });

            self.size = size;
        }
    }

    pub fn element_size(&self) -> usize {
        std::mem::size_of::<T>()
    }
}

// From iced_wgpu
#[derive(Debug)]
pub struct VertexBuffers<T: Pod> {
    pub vertices: Buffer<T>,
    pub indices: Buffer<u16>,
}

impl<T: Pod> VertexBuffers<T> {
    pub fn new(device: &wgpu::Device, init_vertices: &[T], init_indices: &[u16]) -> Self {
        let mut vertices: Buffer<T> = Buffer::new(
            device,
            init_vertices.len(),
            wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        );
        let mut indices: Buffer<u16> = Buffer::new(
            device,
            init_indices.len(),
            wgpu::BufferUsage::INDEX | wgpu::BufferUsage::COPY_DST,
        );
        Self::_write_to(&mut vertices, init_vertices);
        Self::_write_to(&mut indices, init_indices);
        Self { vertices, indices }
    }

    fn _write_to<A: Pod>(buf: &mut Buffer<A>, values: &[A]) {
        buf.buf
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(values));
        buf.buf.unmap();
    }
}
