use std::io::Cursor;
use std::mem::size_of;
use std::num::{NonZeroU32, NonZeroU64};

use bytemuck::cast_slice;
use half::f16;
use image::codecs::hdr::HdrDecoder;
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, Buffer, BufferBinding, BufferBindingType, Color, ColorTargetState,
    CommandEncoder, Device, Extent3d, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    ShaderStages, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDimension, VertexState, BufferUsages, TextureViewDescriptor, SamplerDescriptor,
};

use crate::{Camera};
use crate::plat::load_res;

pub struct GalaxyBox {
    bindgroup: BindGroup,
    pipeline: RenderPipeline,
    quad_buffer: Buffer,
}

impl GalaxyBox {
    pub async fn new(
        device: &Device,
        queue: &Queue,
        camera_buffer: &Buffer,
        target_format: TextureFormat,
    ) -> anyhow::Result<Self> {
        let starmap_res = load_res("res/starmap_2020.hdr").await?;
        let starmap_decoder = HdrDecoder::new(Cursor::new(starmap_res.as_slice()))?;
        let starmap_width = starmap_decoder.metadata().width / 6;
        let starmap_height = starmap_decoder.metadata().height;
        let starmap_native = starmap_decoder.read_image_native()?;

        let mut starmap_samples =
            Vec::with_capacity((6 * starmap_width * starmap_height * 4) as usize);
        for z in 0..6 {
            for y in 0..starmap_height {
                for x in 0..starmap_width {
                    let pos = x + (z * starmap_width) + (y * 6 * starmap_width);
                    let pixel = starmap_native[pos as usize].to_hdr();
                    for ch in 0..3 {
                        starmap_samples.push(f16::from_f32(pixel[ch as usize]));
                    }
                    starmap_samples.push(f16::default());
                }
            }
        }
        drop(starmap_native);

        let starmap_tex = device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: starmap_width,
                    height: starmap_height,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::TEXTURE_BINDING,
            },
            cast_slice(starmap_samples.as_slice()),
        );
        drop(starmap_samples);

        let starmap_view = starmap_tex.create_view(&TextureViewDescriptor {
            label: None,
            format: Some(TextureFormat::Rgba16Float),
            dimension: Some(TextureViewDimension::Cube),
            aspect: TextureAspect::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(6),
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
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
                        view_dimension: wgpu::TextureViewDimension::Cube,
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
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<Camera>() as u64),
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
                    resource: wgpu::BindingResource::TextureView(&starmap_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: camera_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let module = device.create_shader_module(&include_wgsl!("galaxy.wgsl"));
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

        Ok(GalaxyBox {
            pipeline,
            bindgroup,
            quad_buffer,
        })
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
