use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen::prelude::*;
use web_sys::Response;
use anyhow::anyhow;

use log::error;
use winit::platform::web::WindowExtWebSys;
use winit::{event_loop::{EventLoop}, window::WindowBuilder, dpi::PhysicalSize};

pub fn do_main() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init()?;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1024, 768))
        .build(&event_loop)
        .unwrap();

    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
        .and_then(|b| b.append_child(&window.canvas()).ok())
        .ok_or_else(|| anyhow!("error appending canvas to body"))?;

    wasm_bindgen_futures::spawn_local(async {
        let mut cb = match crate::run(window).await {
            Ok(cb) => cb,
            Err(err) => {
                error!("{:?}", err);
                return;
            }
        };

        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(catch, js_namespace = Function, js_name = "prototype.call.call")]
            fn call_catch(this: &JsValue) -> Result<(), JsValue>;
        }

        let _ = call_catch(&Closure::once_into_js(move || {
            event_loop.run(move |event, _, control_flow| {
                if let Err(err) = cb(&event, control_flow) {
                    error!("{err:?}");
                }
            })
        }));
    });
    Ok(())
}

pub async fn load_res(path: &str) -> anyhow::Result<Vec<u8>> {
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
