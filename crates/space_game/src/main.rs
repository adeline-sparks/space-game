use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use log::info;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init().expect("error initializing logger");
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;

        let body = web_sys::window().unwrap().document().unwrap().body().unwrap();
        body.append_child(&window.canvas()).unwrap();
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { window_id, event } = &event {
            assert!(window_id == &window.id());
            info!("Got event: {event:?}");

            if event == &WindowEvent::CloseRequested {
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}
