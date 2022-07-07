use std::mem::size_of;
use std::num::NonZeroU64;
use std::slice;

use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::Vector2;
use wgpu::util::DeviceExt;
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBinding, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureView, TextureViewDimension,
};

use super::StagingBuffer;

/// GPU compute shader for computing a histogram over a texture.
pub struct Histogram {
    /// Number of buckets in the histogram.
    num_buckets: usize,
    /// Buffer storing an array of buckets. Each bucket is a u32.
    buckets_buffer: Buffer,
    /// DownloadQueue for downloading the buckets from the GPU.
    buckets_staging_buffer: StagingBuffer,
    /// BindGroup to use with the pipeline.
    bind_group: BindGroup,
    /// ComputePipeline for executing the histogram shader.
    pipeline: ComputePipeline,
    /// The number of dispatches needed to cover the input texture.
    dispatch_count: Vector2<u32>,
}

/// Uniform variables for the Histogram compute shader.
#[derive(Copy, Clone, Pod, Zeroable, Default, Debug)]
#[repr(C)]
struct HistogramUniforms {
    /// Minimum luminance. Any luminance below this goes to to the first bucket.
    min_lum: f32,
    /// Log2 of the minimum luminance.
    log_min_lum: f32,
    /// Log2 of the maximum luminance. Any luminance above this value goes to the last bucket.
    log_max_lum: f32,
}

impl Histogram {
    /// Initialize a new Histogram compute shader.
    pub fn new(
        device: &Device,
        hdr_view: &TextureView,
        hdr_view_size: Vector2<u32>,
        num_buckets: usize,
        min_lum: f32,
        max_lum: f32,
    ) -> Histogram {
        // Create a bind group layout for the compute pipeline.
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                // The input texture.
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // The bucket buffer.
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // The uniform buffer.
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            NonZeroU64::new(size_of::<HistogramUniforms>() as u64).unwrap(),
                        ),
                    },
                    count: None,
                },
            ],
        });

        // Create a pipeline_layout for the compute shader.
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Compile the ShaderModule.
        let module = device.create_shader_module(include_wgsl!("histogram.wgsl"));

        // Create the compute pipeline.
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "main",
        });

        // Compute the shader's uniforms and upload them to a Buffer.
        let uniforms = HistogramUniforms {
            min_lum,
            log_min_lum: min_lum.log2(),
            log_max_lum: max_lum.log2(),
        };
        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(slice::from_ref(&uniforms)),
            usage: BufferUsages::UNIFORM,
        });

        // Create a buffer to hold the histogram buckets.
        let buckets_buffer_size = num_buckets * size_of::<u32>();
        let buckets_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: buckets_buffer_size as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let buckets_staging_buffer = StagingBuffer::new_read(device, buckets_buffer_size);

        // Create the bind_group using all our buffers.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &buckets_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &uniforms_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        Histogram {
            num_buckets,
            buckets_buffer,
            buckets_staging_buffer,
            bind_group,
            pipeline,
            dispatch_count: hdr_view_size / 16,
        }
    }

    /// Return a reference to the Buffer containing the histogram buckets.
    /// This can be used to pass histogram buckets directly to another shader.
    pub fn buckets_buffer(&self) -> &Buffer {
        &self.buckets_buffer
    }

    /// TODO
    pub fn with_buckets<T>(&mut self, f: impl FnOnce(&[u32]) -> T) -> Option<T> {
        let result = {
            let view = self.buckets_staging_buffer.try_view()?;
            f(cast_slice(&*view))
        };

        self.buckets_staging_buffer.unmap();

        Some(result)
    }

    /// Encode the histogram computation into the `CommandEncoder`.
    pub fn encode(&self, encoder: &mut CommandEncoder) {
        encoder.clear_buffer(&self.buckets_buffer, 0, None);

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.bind_group, &[]);
        compute_pass.dispatch_workgroups(self.dispatch_count.x, self.dispatch_count.y, 1);
        drop(compute_pass);

        let copy_size = self.num_buckets * size_of::<u32>();
        if let Some(buffer) = self.buckets_staging_buffer.try_buffer() {
            encoder.copy_buffer_to_buffer(
                &self.buckets_buffer,
                0,
                buffer,
                0,
                copy_size as u64,
            );
        }
    }

    /// Request to map the readback buffer as soon as it is available. This should be called
    /// immediately after issuing commands to the device, so that the readback buffer is mapped
    /// by the time we render the next frame.
    pub fn map_buffers(&mut self) {
        self.buckets_staging_buffer.map_async();
    }
}
