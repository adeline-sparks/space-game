use std::fs::File;
use std::io::Read;

use log::error;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub fn do_main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024 * 2, 768 * 2))
        .build(&event_loop)
        .unwrap();
    let mut cb = pollster::block_on(crate::run(window))?;
    event_loop.run(move |event, _, control_flow| {
        if let Err(err) = cb(&event, control_flow) {
            error!("{err:?}");
        }
    });
}

pub async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}
