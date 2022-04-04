use std::io::Cursor;
use std::mem::size_of;
use std::num::{NonZeroU32, NonZeroU64};
use std::slice;

use bytemuck::{cast_slice, Pod, Zeroable};
use half::f16;
use image::codecs::hdr::HdrDecoder;
use log::{warn};
use nalgebra::{Vector2, Matrix4, Perspective3, Isometry3, UnitQuaternion, Vector3};
use once_cell::sync::Lazy;
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use wgpu::{
    Backends, Color, DeviceDescriptor, Features, Instance, Limits, LoadOp, Operations, PresentMode,
    RenderPassColorAttachment, RenderPassDescriptor, SurfaceConfiguration, TextureUsages,
    TextureViewDescriptor, RenderPipelineDescriptor, VertexState, PrimitiveState, MultisampleState, FragmentState, ColorTargetState, include_wgsl, Device, Queue, Surface, TextureDescriptor, Extent3d, TextureDimension, TextureFormat, TextureAspect, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, TextureSampleType, SamplerBindingType, BindGroupDescriptor, BindGroupEntry, PipelineLayoutDescriptor, BufferBindingType, BufferUsages, BufferDescriptor, BufferBinding, TextureViewDimension,
};
use winit::dpi::{PhysicalSize};
use winit::event::{Event, WindowEvent, ElementState, DeviceEvent, VirtualKeyCode, KeyboardInput};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};
use anyhow::anyhow;

pub async fn run(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<()> {
    let (device, queue, surface, surface_config) = init_wgpu(&window).await?;
    let module = device.create_shader_module(&include_wgsl!("main.wgsl"));

    let starmap_res = load_res("res/starmap_2020.hdr").await?;
    let starmap_decoder = HdrDecoder::new(Cursor::new(starmap_res.as_slice()))?;
    let starmap_width = starmap_decoder.metadata().width / 6;
    let starmap_height = starmap_decoder.metadata().height;
    let starmap_native = starmap_decoder.read_image_native()?;

    let mut starmap_samples = Vec::with_capacity(
        (6 * starmap_width * starmap_height * 4) as usize
    );
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
        &queue,
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
        cast_slice(starmap_samples.as_slice())
    );
    drop(starmap_samples);

    let starmap_view = starmap_tex.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(TextureFormat::Rgba16Float),
        dimension: Some(TextureViewDimension::Cube),
        aspect: TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: NonZeroU32::new(6),
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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

    #[derive(Copy, Clone, Pod, Zeroable, Default, Debug)]
    #[repr(C)]
    struct Camera {
        inv_view_projection: Matrix4<f32>,
        viewport: Vector2<f32>,
        near: f32,
        far: f32,
    }

    let camera_buffer = device.create_buffer(&BufferDescriptor { 
        label: None, 
        size: size_of::<Camera>() as u64, 
        usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM, 
        mapped_at_creation: false,
    });

    let quad_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: cast_slice::<u16, _>(&[0, 1, 2, 2, 3, 0]),
        usage: BufferUsages::INDEX,
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
                    buffer: &camera_buffer,
                    offset: 0,
                    size: None,
                }),
            },
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

    let mut view = Isometry3::<f64>::default();
    let projection = Perspective3::new(
        surface_config.height as f64 / surface_config.width as f64,
        (60.0f64).to_radians(),
        1.0,
        10.0,
    );

    let mut grabbed = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match &event {
            Event::RedrawRequested(_) => {}
            
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
                return;
            }

            Event::MainEventsCleared => {
                window.request_redraw();
                return;
            }

            Event::WindowEvent { event: WindowEvent::KeyboardInput { 
                input: KeyboardInput { 
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    state: ElementState::Pressed,
                    .. 
                },
                ..
            }, ..} | 
            Event::WindowEvent { event: WindowEvent::Focused(false), .. } => {
                if grabbed {
                    grabbed = false;
                    if let Err(err) = window.set_cursor_grab(false) {
                        warn!("error releasing cursor: {err}");
                    }
                }

                return;
            }

            Event::WindowEvent { event: WindowEvent::MouseInput { state: ElementState::Pressed, .. }, .. } => {
                if !grabbed {
                    if let Err(err) = window.set_cursor_grab(true) {
                        warn!("error grabbing cursor: {err}");
                        return;
                    }
                   
                    grabbed = true;
                }
            }

            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                if !grabbed {
                    return;
                }

                view.append_rotation_mut(&UnitQuaternion::from_scaled_axis(
                    Vector3::new(delta.1, delta.0, 0.0) / 1000.0));
                return;
            }

            _ => {
                return;
            }
        }

        let camera = Camera {
            viewport: Vector2::new(
                surface_config.width as f32,
                surface_config.height as f32,
            ),
            near: projection.znear() as f32,
            far: projection.zfar() as f32,
            inv_view_projection: {
                (view.inverse().to_matrix() * projection.inverse() * *WGPU_TO_OPENGL_MATRIX).cast()
            },
        };
        queue.write_buffer(&camera_buffer, 0, cast_slice(slice::from_ref(&camera)));

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
        render_pass.set_index_buffer(quad_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
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
        limits: Limits::downlevel_webgl2_defaults(),
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

#[cfg(target_arch = "wasm32")]
fn main() {
    use log::error;
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

    wasm_bindgen_futures::spawn_local(async { 
        if let Err(err) = run(event_loop, window).await {
            error!("{:?}", err);
        }
    });
}

#[cfg(target_arch = "wasm32")]
async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Response;
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| anyhow!("error getting window"))?;
    let response = JsFuture::from(window.fetch_with_str(path)).await
        .map_err(|_| anyhow!("fetch failed"))?
        .unchecked_into::<Response>();
    let array_buffer = 
        JsFuture::from(
            response.array_buffer().map_err(|_| anyhow!("array_buffer failed"))?
        )
        .await
        .map_err(|_| anyhow!("array_buffer future failed"))?
        .unchecked_into::<ArrayBuffer>();
    Ok(Uint8Array::new(&array_buffer).to_vec())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024*2, 768*2))
        .build(&event_loop)
        .unwrap();
    pollster::block_on(run(event_loop, window))
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    use std::{fs::File, io::Read};

    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}

static OPENGL_TO_WGPU_MATRIX: Matrix4<f64> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

static WGPU_TO_OPENGL_MATRIX: Lazy<Matrix4<f64>> = Lazy::new(|| {
    OPENGL_TO_WGPU_MATRIX.try_inverse().unwrap()
});