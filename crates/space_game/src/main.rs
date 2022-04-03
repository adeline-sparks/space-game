use std::io::Cursor;

use exr::prelude::{ReadChannels, ReadLayers};
use wgpu::{
    Backends, Color, DeviceDescriptor, Features, Instance, Limits, LoadOp, Operations, PresentMode,
    RenderPassColorAttachment, RenderPassDescriptor, SurfaceConfiguration, TextureUsages,
    TextureViewDescriptor, RenderPipelineDescriptor, VertexState, PrimitiveState, MultisampleState, FragmentState, ColorTargetState, include_wgsl,
};
use winit::dpi::{PhysicalSize};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Window};

pub async fn run(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<()> {
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(Backends::all);
    let instance = Instance::new(backends);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter =
        wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, Some(&surface))
            .await
            .expect("error finding adapter");

    let device_desc = DeviceDescriptor {
        label: None,
        features: Features::empty(),
        limits: Limits::downlevel_webgl2_defaults(),
    };
    let (device, queue) = adapter
        .request_device(&device_desc, None)
        .await
        .expect("error requesting device");

    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width: size.width,
        height: size.height,
        present_mode: PresentMode::Fifo,
    };
    surface.configure(&device, &surface_config);

    let starmap_image = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |size, _| {
                (vec![0f32; size.width() * size.height() * 4], size)
            },
            |(buf, size), pos, (r, g, b, a): (f32, f32, f32, f32)| {
                let pos = 4 * (pos.x() + size.width() * pos.y());
                buf[pos..pos+4].copy_from_slice(&[r, g, b, a]);
            }
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(Cursor::new(load_res("res/starmap_2020_4k.exr").await?))?;

    let module = device.create_shader_module(&include_wgsl!("main.wgsl"));

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: None,
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
        render_pass.draw(0..3, 0..1);
        drop(render_pass);

        queue.submit([encoder.finish()]);
        surface_texture.present();
    });
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
