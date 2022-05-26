use std::{mem::size_of};


use nalgebra::Vector2;
use wgpu::{BindGroup, Buffer, Device, TextureView, BindGroupLayoutEntry, ShaderStages, TextureSampleType, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, PipelineLayoutDescriptor, include_wgsl, BufferUsages, CommandEncoder, TextureViewDimension, BindingType, BufferBindingType, BufferBinding, ComputePipeline, ComputePipelineDescriptor, ComputePassDescriptor, BufferDescriptor};

pub struct Histogram {
    buffer: Buffer,
    bindgroup: BindGroup,
    pipeline: ComputePipeline,
    dispatch_count: Vector2<u32>,
}

impl Histogram {
    pub fn new(
        device: &Device,
        hdr_view: &TextureView,
        hdr_tex_size: Vector2<u32>,
    ) -> Histogram {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size_of::<[u32; 256]>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bindgroup_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
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
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { 
                        ty: BufferBindingType::Storage { 
                            read_only: false, 
                        }, 
                        has_dynamic_offset: false, 
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
        });

        let bindgroup = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bindgroup_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }
            ],
        });

        let module = device.create_shader_module(&include_wgsl!("tonemap_histogram.wgsl"));
        
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bindgroup_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "main",
        });

        Histogram {
            buffer,
            bindgroup, 
            pipeline, 
            dispatch_count: hdr_tex_size / 16,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn encode(&self, encoder: &mut CommandEncoder) {
        encoder.clear_buffer(&self.buffer, 0, None);
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
        });
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.bindgroup, &[]);
        compute_pass.dispatch_workgroups(self.dispatch_count.x, self.dispatch_count.y, 1);
        drop(compute_pass);
    }
}