use std::{num::NonZeroU64, mem::{self}};

use bytemuck::{Pod, Zeroable};
use nalgebra::Vector2;
use wgpu::{ComputePipeline, BindGroup, Device, TextureFormat, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, ShaderStages, TextureSampleType, TextureViewDimension, BufferBindingType, BindGroupDescriptor, BindGroupEntry, TextureView, BindingResource, BufferDescriptor, BufferUsages, BufferBinding, PipelineLayoutDescriptor, Buffer, ComputePipelineDescriptor, include_wgsl, StorageTextureAccess, CommandEncoder, ComputePassDescriptor};

use crate::Camera;

pub struct Tonemap {
    bindgroup: BindGroup,
    histogram_pipeline: ComputePipeline,
    exposure_pipeline: ComputePipeline,
    tonemap_pipeline: ComputePipeline,
    dispatch_size: Vector2<u32>,
}

const NUM_BUCKETS: usize = 256;
const WORKGROUP_SIZE: Vector2<u32> = Vector2::new(16, 16);

#[derive(Copy, Clone, Pod, Zeroable, Debug)]
#[repr(C)]
struct TonemapState {
    exposure: f32,
    buckets: [u32; NUM_BUCKETS],
}

impl Tonemap {
    pub fn new(
        device: &Device,
        source: &TextureView,
        source_size: Vector2<u32>,
        dest: &TextureView,
        dest_format: TextureFormat,
        camera_buffer: &Buffer,
    ) -> anyhow::Result<Self> {
        let tonemap_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: mem::size_of::<TonemapState>() as u64,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let module = device.create_shader_module(&include_wgsl!("tonemap.wgsl"));

        let bindgroup_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture { 
                        sample_type: TextureSampleType::Float {
                            filterable: false,
                        },
                        view_dimension: TextureViewDimension::D2, 
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage {
                            read_only: true,
                        },
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(mem::size_of::<Camera>() as u64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { 
                        ty: BufferBindingType::Storage { 
                            read_only: false, 
                        }, 
                        has_dynamic_offset: false, 
                        min_binding_size: NonZeroU64::new(mem::size_of::<TonemapState>() as u64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture { 
                        access: StorageTextureAccess::WriteOnly, 
                        format: dest_format, 
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                }
            ]
        });

        let bindgroup = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bindgroup_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(source),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &camera_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &tonemap_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(dest),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bindgroup_layout],
            push_constant_ranges: &[],
        });

        let histogram_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "histogram_main",
        });

        let exposure_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "exposure_main",
        });

        let tonemap_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "tonemap_main",
        });

        let dispatch_size = source_size / 16;
        Ok(Tonemap { bindgroup, histogram_pipeline, exposure_pipeline, tonemap_pipeline, dispatch_size })
    }

    pub fn draw(&self, encoder: &mut CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
        });
        compute_pass.set_bind_group(0, &self.bindgroup, &[]);
        compute_pass.set_pipeline(&self.histogram_pipeline);
        compute_pass.dispatch(self.dispatch_size.x, self.dispatch_size.y, 1);
        compute_pass.set_pipeline(&self.exposure_pipeline);
        compute_pass.dispatch(1, 1, 1);
        compute_pass.set_pipeline(&self.tonemap_pipeline);
        compute_pass.dispatch(self.dispatch_size.x, self.dispatch_size.y, 1);
    }
}