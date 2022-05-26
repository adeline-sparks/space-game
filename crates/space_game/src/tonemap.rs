use std::{num::NonZeroU32, mem::size_of};

use bytemuck::cast_slice;
use nalgebra::Vector2;
use wgpu::{BindGroup, RenderPipeline, Buffer, Device, TextureView, SamplerDescriptor, BindGroupLayoutEntry, ShaderStages, TextureSampleType, SamplerBindingType, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, PipelineLayoutDescriptor, VertexState, PrimitiveState, MultisampleState, FragmentState, ColorTargetState, RenderPipelineDescriptor, include_wgsl, util::{DeviceExt, BufferInitDescriptor}, BufferUsages, TextureFormat, CommandEncoder, RenderPassDescriptor, RenderPassColorAttachment, Operations, Color, LoadOp, TextureViewDescriptor, TextureAspect, Texture, TextureViewDimension, BindingType, BufferBindingType, BufferBinding, ComputePipeline, ComputePipelineDescriptor, ComputePassDescriptor, BufferDescriptor};

pub struct Tonemap {
    histogram_buffer: Buffer,
    histogram_bindgroup: BindGroup,
    histogram_pipeline: ComputePipeline,
    histogram_dispatches: Vector2<u32>,
    render_bindgroup: BindGroup,
    render_pipeline: RenderPipeline,
    render_indices: Buffer,
}

impl Tonemap {
    pub fn new(
        device: &Device,
        hdr_tex: &Texture,
        hdr_tex_size: Vector2<u32>,
        hdr_format: TextureFormat,
        target_format: TextureFormat,
    ) -> anyhow::Result<Tonemap> {
        let histogram_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size_of::<[u32; 256]>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hdr_view = hdr_tex.create_view(&TextureViewDescriptor {
            label: None,
            format: Some(hdr_format),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(1),
        });

        let (histogram_bindgroup, histogram_pipeline, histogram_dispatches) = Self::make_histogram_pipeline(
            device,
            &hdr_view,
            hdr_tex_size,
            &histogram_buffer,
        );

        let (render_bindgroup, render_pipeline, render_indices) = Self::make_render_pipeline(
            device,
            &hdr_view,
            &histogram_buffer,
            target_format,
        );

        Ok(Tonemap { 
            histogram_buffer,
            histogram_bindgroup, 
            histogram_pipeline, 
            histogram_dispatches,
            render_bindgroup, 
            render_pipeline, 
            render_indices
         })
    }

    fn make_histogram_pipeline(
        device: &Device,
        hdr_view: &TextureView,
        hdr_tex_size: Vector2<u32>,
        histogram_buffer: &Buffer,
    ) -> (BindGroup, ComputePipeline, Vector2<u32>) {
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
                        buffer: histogram_buffer,
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

        (bindgroup, pipeline, hdr_tex_size / 16)
    }

    fn make_render_pipeline(
        device: &Device,
        hdr_view: &TextureView,
        histogram_buffer: &Buffer,
        target_format: TextureFormat,
    ) -> (BindGroup, RenderPipeline, Buffer) {
        let hdr_sampler = device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
        });

        let bindgroup_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer { 
                        ty: BufferBindingType::Storage { 
                            read_only: true, 
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
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: histogram_buffer,
                        offset: 0,
                        size: None,
                    }),
                }
            ],
        });

        let module = device.create_shader_module(&include_wgsl!("tonemap_render.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bindgroup_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &module,
                entry_point: "vert_main",
                buffers: &[],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &module,
                entry_point: "frag_main",
                targets: &[ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            multiview: None,
        });

        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: cast_slice::<u16, _>(&[0, 1, 2, 2, 3, 0]),
            usage: BufferUsages::INDEX,
        });

        (bindgroup, pipeline, indices)
    }

    pub fn draw(&self, encoder: &mut CommandEncoder, target: &TextureView) {
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
        });
        compute_pass.set_pipeline(&self.histogram_pipeline);
        compute_pass.set_bind_group(0, &self.histogram_bindgroup, &[]);
        compute_pass.dispatch(self.histogram_dispatches.x, self.histogram_dispatches.y, 1);
        drop(compute_pass);

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bindgroup, &[]);
        render_pass.set_index_buffer(self.render_indices.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
        drop(render_pass);
    }
}