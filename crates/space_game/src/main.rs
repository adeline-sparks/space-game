use wgpu::{Backends, Instance};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use log::info;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    #[cfg(target_arch = "wasm32")]
    console_log::init().expect("error initializing logger");

    let fut = async {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop).unwrap();

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.body())
                .and_then(|b| b.append_child(&window.canvas()).ok())
                .expect("error appending canvas to body");
        }

        let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(Backends::all);
        let instance = Instance::new(backends);
        let surface = unsafe { instance.create_surface(&window) };
        let adapter =
            wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, Some(&surface))
                .await
                .expect("error finding adapter");
        let adapter_info = adapter.get_info();
        info!("Using {} ({:?})", adapter_info.name, adapter_info.backend);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            if let Event::WindowEvent { window_id, event } = &event {
                assert!(window_id == &window.id());

                if event == &WindowEvent::CloseRequested {
                    *control_flow = ControlFlow::Exit;
                }
            }
        });
    };

    #[cfg(not(target_arch = "wasm32"))]
    pollster::block_on(fut);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut);
}
