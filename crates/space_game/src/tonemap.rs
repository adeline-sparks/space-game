use std::num::NonZeroU32;

use bytemuck::cast_slice;
use wgpu::{BindGroup, RenderPipeline, Buffer, Device, TextureView, SamplerDescriptor, BindGroupLayoutEntry, ShaderStages, TextureSampleType, SamplerBindingType, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, PipelineLayoutDescriptor, VertexState, PrimitiveState, MultisampleState, FragmentState, ColorTargetState, RenderPipelineDescriptor, include_wgsl, util::{DeviceExt, BufferInitDescriptor}, BufferUsages, TextureFormat, CommandEncoder, RenderPassDescriptor, RenderPassColorAttachment, Operations, Color, LoadOp, TextureViewDescriptor, TextureAspect, Texture, TextureViewDimension};

pub struct Tonemap {
    bindgroup: BindGroup,
    pipeline: RenderPipeline,
    quad_buffer: Buffer,
}

impl Tonemap {
    pub fn new(
        device: &Device,
        hdr_tex: &Texture,
        hdr_format: TextureFormat,
        target_format: TextureFormat,
    ) -> anyhow::Result<Tonemap> {
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
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
                    resource: wgpu::BindingResource::TextureView(&hdr_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&hdr_sampler),
                },
            ],
        });

        let module = device.create_shader_module(&include_wgsl!("tonemap.wgsl"));
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

        let quad_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: cast_slice::<u16, _>(&[0, 1, 2, 2, 3, 0]),
            usage: BufferUsages::INDEX,
        });

        Ok(Tonemap { bindgroup, pipeline, quad_buffer })
    }

    pub fn draw(&self, encoder: &mut CommandEncoder, target: &TextureView) {
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
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bindgroup, &[]);
        render_pass.set_index_buffer(self.quad_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }
}