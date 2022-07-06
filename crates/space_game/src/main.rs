use anyhow::anyhow;
use bytemuck::{Pod, Zeroable};
use log::{info, warn};
use nalgebra::{Isometry3, Matrix4, UnitQuaternion, Vector2, Vector3};
use plat::EventHandler;
use wgpu::{
    Backends, Device, DeviceDescriptor, Features, Instance, Limits, PresentMode, Queue, Surface,
    SurfaceConfiguration, TextureUsages, TextureViewDescriptor,
};

use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::window::Window;

mod plat;
mod render;

fn main() -> anyhow::Result<()> {
    plat::do_main()
}

use crate::render::Renderer;

#[derive(Copy, Clone, Pod, Zeroable, Default, Debug)]
#[repr(C)]
struct Camera {
    inv_view_projection: Matrix4<f32>,
    viewport: Vector2<f32>,
    near: f32,
    far: f32,
}

pub async fn run(window: Window) -> anyhow::Result<EventHandler> {
    let (device, queue, surface, surface_config) = init_wgpu(&window).await?;
    let mut renderer = Renderer::new(
        &device,
        &queue,
        Vector2::new(surface_config.width as u32, surface_config.height as u32),
        surface_config.format,
    )
    .await?;

    let mut view = Isometry3::<f64>::default();

    let mut grabbed = false;
    info!("Initialized");
    Ok(Box::new(move |event, control_flow| {
        *control_flow = ControlFlow::Poll;

        match &event {
            Event::RedrawRequested(_) => {}

            Event::MainEventsCleared => {
                window.request_redraw();
                return Ok(());
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                return Ok(());
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
                    window.set_cursor_visible(true);
                    if let Err(err) = window.set_cursor_grab(false) {
                        warn!("error releasing cursor: {err}");
                    }
                }

                return Ok(());
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
                        return Ok(());
                    }

                    window.set_cursor_visible(false);
                    grabbed = true;
                }

                return Ok(());
            }

            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if !grabbed {
                    return Ok(());
                }

                view.append_rotation_mut(&UnitQuaternion::from_scaled_axis(
                    Vector3::new(delta.1, delta.0, 0.0) / 1000.0,
                ));
                return Ok(());
            }

            _ => {
                return Ok(());
            }
        }

        let surface_texture = surface.get_current_texture().unwrap();
        let surface_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        renderer.draw(&device, &queue, &surface_view, &view);
        surface_texture.present();
        Ok(())
    }))
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
        limits: Limits::downlevel_defaults(),
    };
    let (device, queue) = adapter.request_device(&device_desc, None).await?;

    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: *surface.get_supported_formats(&adapter).get(0).unwrap(),
        width: size.width,
        height: size.height,
        present_mode: PresentMode::Fifo,
    };
    surface.configure(&device, &surface_config);

    Ok((device, queue, surface, surface_config))
}
