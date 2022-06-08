use std::{mem::size_of, sync::{atomic::{AtomicBool, Ordering}, Arc}};

use bytemuck::cast_slice_mut;
use nalgebra::Vector2;
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBinding, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureView, TextureViewDimension, MapMode,
};

pub struct Histogram {
    buffer: Buffer,
    read_buffer: Buffer,
    bindgroup: BindGroup,
    pipeline: ComputePipeline,
    dispatch_count: Vector2<u32>,
    read_buffer_mapped: Arc<AtomicBool>,
    read_buffer_contents: [u32; NUM_BUCKETS],
}

const NUM_BUCKETS: usize = 256;
const BUFFER_SIZE: usize = size_of::<u32>() * NUM_BUCKETS;

impl Histogram {
    pub fn new(device: &Device, hdr_view: &TextureView, hdr_tex_size: Vector2<u32>) -> Histogram {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: BUFFER_SIZE as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let read_buffer = device.create_buffer(&BufferDescriptor { 
            label: None, 
            size: BUFFER_SIZE as u64, 
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
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
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
                },
            ],
        });

        let module = device.create_shader_module(&include_wgsl!("histogram.wgsl"));

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
            read_buffer,
            bindgroup,
            pipeline,
            dispatch_count: hdr_tex_size / 16,
            read_buffer_mapped: Arc::new(AtomicBool::new(false)),
            read_buffer_contents: [0u32; NUM_BUCKETS],
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn dispatch(&mut self, encoder: &mut CommandEncoder) {
        if self.read_buffer_mapped.swap(false, Ordering::Acquire) {
            cast_slice_mut(&mut self.read_buffer_contents).copy_from_slice(&*self.read_buffer.slice(..).get_mapped_range());
            println!("Got: {:?}", self.read_buffer_contents);
            self.read_buffer.unmap();
        } else {
            println!("No data available");
        }

        encoder.clear_buffer(&self.buffer, 0, None);

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.bindgroup, &[]);
        compute_pass.dispatch_workgroups(self.dispatch_count.x, self.dispatch_count.y, 1);
        drop(compute_pass);

        encoder.copy_buffer_to_buffer(&self.buffer, 0, &self.read_buffer, 0, BUFFER_SIZE as u64);
    }

    pub fn map(&self) {
        let slice = self.read_buffer.slice(..);
        let mapped_flag = self.read_buffer_mapped.clone();
        slice.map_async(MapMode::Read, move |result| { 
            assert!(result.is_ok());
            mapped_flag.store(true, Ordering::Release);
        });
    }
}
