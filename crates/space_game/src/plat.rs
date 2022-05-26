use winit::event::Event;
use winit::event_loop::ControlFlow;

pub type EventHandler = Box<dyn FnMut(&Event<()>, &mut ControlFlow) -> anyhow::Result<()>>;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
