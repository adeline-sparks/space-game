use std::mem::size_of;

use std::slice;

use anyhow::anyhow;
use bytemuck::{cast_slice, Pod, Zeroable};
use log::{info, warn};
use nalgebra::{Isometry3, Matrix4, Perspective3, UnitQuaternion, Vector2, Vector3};
use once_cell::sync::Lazy;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Backends, BufferDescriptor, BufferUsages, Device, DeviceDescriptor, Features, Instance, Limits,
    PresentMode, Queue, Surface, SurfaceConfiguration, TextureUsages, TextureViewDescriptor,
};
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::window::{Window, WindowBuilder};

mod galaxy;

use galaxy::GalaxyBox;

#[derive(Copy, Clone, Pod, Zeroable, Default, Debug)]
#[repr(C)]
struct Camera {
    inv_view_projection: Matrix4<f32>,
    viewport: Vector2<f32>,
    near: f32,
    far: f32,
}

pub async fn run(event_loop: EventLoop<()>, window: Window) -> anyhow::Result<()> {
    let (device, queue, surface, surface_config) = init_wgpu(&window).await?;

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

    let galaxy_box = GalaxyBox::new(&device, &queue, &camera_buffer, surface_config.format).await?;

    let mut view = Isometry3::<f64>::default();
    let projection = Perspective3::new(
        surface_config.height as f64 / surface_config.width as f64,
        (60.0f64).to_radians(),
        1.0,
        10.0,
    );

    let mut grabbed = false;
    info!("Initialized");
    run_event_loop(event_loop, move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match &event {
            Event::RedrawRequested(_) => {}

            Event::MainEventsCleared => {
                window.request_redraw();
                return;
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                return;
            }

            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            }
            | Event::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } => {
                if grabbed {
                    grabbed = false;
                    if let Err(err) = window.set_cursor_grab(false) {
                        warn!("error releasing cursor: {err}");
                    }
                }

                return;
            }

            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if !grabbed {
                    if let Err(err) = window.set_cursor_grab(true) {
                        warn!("error grabbing cursor: {err}");
                        return;
                    }

                    grabbed = true;
                }
            }

            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if !grabbed {
                    return;
                }

                view.append_rotation_mut(&UnitQuaternion::from_scaled_axis(
                    Vector3::new(delta.1, delta.0, 0.0) / 1000.0,
                ));
                return;
            }

            _ => {
                return;
            }
        }

        let camera = Camera {
            viewport: Vector2::new(surface_config.width as f32, surface_config.height as f32),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        galaxy_box.draw(&mut encoder, &quad_buffer, &surface_view);

        queue.submit([encoder.finish()]);
        surface_texture.present();
    });

    Ok(())
}

async fn init_wgpu(
    window: &Window,
) -> anyhow::Result<(Device, Queue, Surface, SurfaceConfiguration)> {
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
    let (device, queue) = adapter.request_device(&device_desc, None).await?;

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

static OPENGL_TO_WGPU_MATRIX: Matrix4<f64> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
);

static WGPU_TO_OPENGL_MATRIX: Lazy<Matrix4<f64>> =
    Lazy::new(|| OPENGL_TO_WGPU_MATRIX.try_inverse().unwrap());

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024 * 2, 768 * 2))
        .build(&event_loop)
        .unwrap();
    pollster::block_on(run(event_loop, window))
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    use std::fs::File;
    use std::io::Read;

    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(not(target_arch = "wasm32"))]
fn run_event_loop(
    event_loop: EventLoop<()>,
    event_handler: impl FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow) + 'static,
) {
    event_loop.run(event_handler);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use log::error;
    use winit::platform::web::WindowExtWebSys;

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("error initializing logger");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024, 768))
        .build(&event_loop)
        .unwrap();

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
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Response;

    let window = web_sys::window().ok_or_else(|| anyhow!("error getting window"))?;
    let response = JsFuture::from(window.fetch_with_str(path))
        .await
        .map_err(|_| anyhow!("fetch failed"))?
        .unchecked_into::<Response>();
    let array_buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|_| anyhow!("array_buffer failed"))?,
    )
    .await
    .map_err(|_| anyhow!("array_buffer future failed"))?
    .unchecked_into::<ArrayBuffer>();
    Ok(Uint8Array::new(&array_buffer).to_vec())
}

#[cfg(target_arch = "wasm32")]
fn run_event_loop(
    event_loop: EventLoop<()>,
    event_handler: impl FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow) + 'static,
) {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(catch, js_namespace = Function, js_name = "prototype.call.call")]
        fn call_catch(this: &JsValue) -> Result<(), JsValue>;
    }

    let _ = call_catch(&Closure::once_into_js(move || {
        event_loop.run(event_handler)
    }));
}
