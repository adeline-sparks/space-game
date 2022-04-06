use std::{fs::File, io::Read};

use winit::{event_loop::{EventLoop, EventLoopWindowTarget, ControlFlow}, window::WindowBuilder, dpi::PhysicalSize, event::Event};

pub fn do_main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024 * 2, 768 * 2))
        .build(&event_loop)
        .unwrap();
    pollster::block_on(crate::run(event_loop, window))
}

pub async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn run_event_loop(
    event_loop: EventLoop<()>,
    event_handler: impl FnMut(Event<'_, ()>, &EventLoopWindowTarget<()>, &mut ControlFlow) + 'static,
) {
    event_loop.run(event_handler);
}