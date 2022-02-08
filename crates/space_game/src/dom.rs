use futures::future::FusedFuture;
use futures::{select, Future, FutureExt};
use js_sys::{Function, Promise};
use log::error;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    AddEventListenerOptions, BinaryType, Document, EventTarget, HtmlCanvasElement,
    HtmlImageElement, WebSocket, Window,
};

mod input;
pub use input::{key_consts, InputEventListener, Key};

pub async fn content_loaded() -> Result<(), JsValue> {
    if document()?.ready_state() == "loading" {
        await_event(window()?.as_ref(), "DOMContentLoaded")?.await;
    }

    Ok(())
}

pub async fn animation_frame() -> Result<f64, JsValue> {
    let (cb, future) = make_callback_future();
    window()?.request_animation_frame(&cb)?;

    future
        .await
        .as_f64()
        .ok_or("Failed to cast timestamp to f64".into())
}

pub async fn load_image(src: &str) -> Result<HtmlImageElement, JsValue> {
    let image = web_sys::HtmlImageElement::new()?;
    image.set_src(src);

    select! {
        _ = await_event(&image, "load")? => Ok(image),
        val = await_event(&image, "error")? => Err(val),
    }
}

pub async fn open_websocket(url: &str) -> Result<WebSocket, JsValue> {
    let ws = WebSocket::new(url)?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    select! {
        _ = await_event(&ws, "open")? => Ok(ws),
        val = await_event(&ws, "error")? => Err(val),
    }
}

pub fn await_event(
    target: &EventTarget,
    type_: &str,
) -> Result<impl FusedFuture<Output = JsValue>, JsValue> {
    let (cb, future) = make_callback_future();
    target.add_event_listener_with_callback_and_add_event_listener_options(
        type_,
        &cb,
        &AddEventListenerOptions::new().once(true),
    )?;
    Ok(future)
}

fn make_callback_future() -> (Function, impl FusedFuture<Output = JsValue>) {
    let mut resolve_opt = None;
    let future = JsFuture::from(Promise::new(&mut |resolve, _reject| {
        resolve_opt = Some(resolve);
    }));

    (resolve_opt.unwrap(), async { future.await.unwrap() }.fuse())
}

pub fn spawn(fut: impl Future<Output = Result<(), JsValue>> + 'static) {
    spawn_local(async move {
        if let Err(e) = fut.await {
            error!("Future died");
            web_sys::console::log_1(&e);
        }
    });
}

pub fn get_canvas(element_id: &str) -> Result<HtmlCanvasElement, JsValue> {
    Ok(document()?
        .get_element_by_id(element_id)
        .ok_or_else(|| JsValue::from(format!("get_element_by_id failed for `{}`", element_id)))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?)
}

fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| "Global `window` does not exist".into())
}

fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| "Global `document` does not exist".into())
}
