use std::io::Cursor;
use std::num::NonZeroU32;

use bytemuck::{cast_slice};
use exr::prelude::{ReadChannels, ReadLayers};
use nalgebra::Vector2;
use wgpu::{
    Backends, Color, DeviceDescriptor, Features, Instance, Limits, LoadOp, Operations, PresentMode,
    RenderPassColorAttachment, RenderPassDescriptor, SurfaceConfiguration, TextureUsages,
    TextureViewDescriptor, RenderPipelineDescriptor, VertexState, PrimitiveState, MultisampleState, FragmentState, ColorTargetState, include_wgsl, Device, Queue, Surface, TextureDescriptor, Extent3d, TextureDimension, TextureFormat, ImageCopyTexture, TextureAspect, Origin3d, ImageDataLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, TextureSampleType, SamplerBindingType, BindGroupDescriptor, BindGroupEntry, PipelineLayoutDescriptor,
};
use winit::dpi::{PhysicalSize};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};
use anyhow::anyhow;

pub async fn run(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<()> {
    let (device, queue, surface, surface_config) = init_wgpu(&window).await?;
    let module = device.create_shader_module(&include_wgsl!("main.wgsl"));

    let starmap_image = read_exr(load_res("res/starmap_2020_4k.exr").await?.as_slice())?;
    let starmap_tex_size = Extent3d {
        width: starmap_image.size.x,
        height: starmap_image.size.y,
        depth_or_array_layers: 1,
    };
    let starmap_tex = device.create_texture(&TextureDescriptor {
        label: None,
        size: starmap_tex_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba32Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    });

    queue.write_texture(
        ImageCopyTexture {
            texture: &starmap_tex,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        }, 
        cast_slice(starmap_image.data.as_slice()), 
        ImageDataLayout { 
            offset: 0, 
            bytes_per_row: NonZeroU32::new(4 * 4 * starmap_image.size.x), 
            rows_per_image: NonZeroU32::new(starmap_image.size.y),
        },
        starmap_tex_size,
    );
    let starmap_view = starmap_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: None,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
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
                    sample_type: TextureSampleType::Float { filterable: false }, 
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
                resource: wgpu::BindingResource::TextureView(&starmap_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }
        ],
    });

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
                format: surface_config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }),
        multiview: None,
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if matches!(
            &event,
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
        ) {
            *control_flow = ControlFlow::Exit;
            return;
        }

        if !matches!(&event, Event::RedrawRequested(_)) {
            return;
        }

        let surface_texture = surface.get_current_texture().unwrap();
        let surface_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[RenderPassColorAttachment {
                view: &surface_view,
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
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bindgroup, &[]);
        render_pass.draw(0..3, 0..1);
        drop(render_pass);

        queue.submit([encoder.finish()]);
        surface_texture.present();
    });
}

async fn init_wgpu(window: &Window) -> anyhow::Result<(Device, Queue, Surface, SurfaceConfiguration)> {
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(Backends::all);
    let instance = Instance::new(backends);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter =
        wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, Some(&surface))
            .await
            .ok_or_else(|| anyhow!("error finding adapter"))?;

    let device_desc = DeviceDescriptor {
        label: None,
        features: Features::empty(),
        limits: Limits::default(),
    };
    let (device, queue) = adapter
        .request_device(&device_desc, None)
        .await?;

    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width: size.width,
        height: size.height,
        present_mode: PresentMode::Fifo,
    };
    surface.configure(&device, &surface_config);

    Ok((device, queue, surface, surface_config))
}

struct Image {
    data: Vec<f32>,
    size: Vector2<u32>,
}

fn read_exr(bytes: &[u8]) -> anyhow::Result<Image> {
    Ok(exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(|dims, _| Image {
            data: vec![0f32; 4 * dims.area()],
            size: Vector2::new(dims.width() as u32, dims.height() as u32),
        }, 
        |image, coord, (r, g, b, a)| {
            let pos = 4 * (coord.x() + (image.size.x as usize) * coord.y());
            image.data[pos..pos+4].copy_from_slice(&[r, g, b, a]);
        })
        .first_valid_layer()
        .all_attributes()
        .from_buffered(Cursor::new(bytes))?
        .layer_data
        .channel_data
        .pixels
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use winit::platform::web::WindowExtWebSys;

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("error initializing logger");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_inner_size(PhysicalSize::new(1024, 768)).build(&event_loop).unwrap();

    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
        .and_then(|b| b.append_child(&window.canvas()).ok())
        .expect("error appending canvas to body");

    wasm_bindgen_futures::spawn_local(run(event_loop, window));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024, 768))
        .build(&event_loop)
        .unwrap();
    pollster::block_on(run(event_loop, window))
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    use std::{fs::File, io::Read};

    let mut buf = Vec::new();
    File::open(format!("crates/space_game/{path}"))?.read_to_end(&mut buf)?;
    Ok(buf)
}
