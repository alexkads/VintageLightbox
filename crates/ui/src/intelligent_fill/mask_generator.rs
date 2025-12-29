//! Mask Generator
//!
//! Gera máscaras de rotação usando wgpu compute shaders.
//! A máscara indica quais pixels estão fora dos bounds originais
//! após a rotação e precisam ser preenchidos.


/// Parâmetros para geração de máscara (deve corresponder ao layout WGSL)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaskParams {
    pub angle_rad: f32,
    pub crop_x: f32,
    pub crop_y: f32,
    pub crop_width: f32,
    pub crop_height: f32,
    pub image_width: f32,
    pub image_height: f32,
    pub _padding: f32,
}

impl MaskParams {
    pub fn new(
        angle_degrees: f32,
        crop_x: f32,
        crop_y: f32,
        crop_width: f32,
        crop_height: f32,
        image_width: u32,
        image_height: u32,
    ) -> Self {
        Self {
            angle_rad: angle_degrees.to_radians(),
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            image_width: image_width as f32,
            image_height: image_height as f32,
            _padding: 0.0,
        }
    }
}

/// Recursos GPU para geração de máscara
pub struct MaskResources {
    pub params_buffer: wgpu::Buffer,
    pub output_texture: wgpu::Texture,
    pub output_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub padded_bytes_per_row: u32,
}

/// Gerador de máscaras de rotação via GPU
pub struct MaskGenerator {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MaskGenerator {
    /// Cria um novo MaskGenerator
    pub fn new(device: &wgpu::Device) -> Self {
        let shader_source = include_str!("../shaders/rotation_mask.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rotation Mask Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mask Bind Group Layout"),
            entries: &[
                // Params uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output mask texture (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mask Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Mask Generation Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }

    /// Cria recursos GPU para uma dimensão específica
    pub fn create_resources(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> MaskResources {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mask Params Buffer"),
            size: std::mem::size_of::<MaskParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Mask Output Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Buffer alignment: 256 bytes
        let unpadded_bytes_per_row = width;
        let padded_bytes_per_row = (unpadded_bytes_per_row + 255) & !255;

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mask Output Buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mask Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
            ],
        });

        MaskResources {
            params_buffer,
            output_texture,
            output_buffer,
            bind_group,
            padded_bytes_per_row,
        }
    }

    /// Gera a máscara de rotação
    pub fn generate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &MaskResources,
        params: &MaskParams,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        // Atualizar parâmetros
        queue.write_buffer(
            &resources.params_buffer,
            0,
            bytemuck::bytes_of(params),
        );

        // Criar command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Mask Generation Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Mask Generation Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &resources.bind_group, &[]);

            // Dispatch: 16x16 workgroups
            let workgroups_x = width.div_ceil(16);
            let workgroups_y = height.div_ceil(16);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // Copiar textura para buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &resources.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &resources.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(resources.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Ler resultado
        let buffer_slice = resources.output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().expect("Failed to map mask buffer");

        let data = buffer_slice.get_mapped_range();

        // Remover padding
        let mut result = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            let row_start = (y * resources.padded_bytes_per_row) as usize;
            let row_end = row_start + width as usize;
            result.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        resources.output_buffer.unmap();

        result
    }

    /// Gera máscara em CPU (fallback)
    pub fn generate_cpu(params: &MaskParams, width: u32, height: u32) -> Vec<u8> {
        let cos_a = (-params.angle_rad).cos();
        let sin_a = (-params.angle_rad).sin();

        let mut mask = vec![0u8; (width * height) as usize];

        let center_x = 0.5;
        let center_y = 0.5;

        for y in 0..height {
            for x in 0..width {
                // Normalizar para 0-1
                let nx = x as f32 / width as f32;
                let ny = y as f32 / height as f32;

                // Verificar se está dentro do crop
                let in_crop = nx >= params.crop_x
                    && nx <= params.crop_x + params.crop_width
                    && ny >= params.crop_y
                    && ny <= params.crop_y + params.crop_height;

                if !in_crop {
                    continue;
                }

                // Transformar para coordenadas rotacionadas
                let pos_x = nx - center_x;
                let pos_y = ny - center_y;

                // Correção de aspect ratio
                let aspect = params.image_width / params.image_height;
                let pos_corrected_x = pos_x * aspect;
                let pos_corrected_y = pos_y;

                // Rotação inversa
                let rotated_x = pos_corrected_x * cos_a - pos_corrected_y * sin_a;
                let rotated_y = pos_corrected_x * sin_a + pos_corrected_y * cos_a;

                // Restaurar aspect ratio
                let source_x = rotated_x / aspect + center_x;
                let source_y = rotated_y + center_y;

                // Verificar se está dentro dos bounds
                let is_valid = (0.0..=1.0).contains(&source_x)
                    && (0.0..=1.0).contains(&source_y);

                if !is_valid {
                    mask[(y * width + x) as usize] = 255;
                }
            }
        }

        mask
    }
}
