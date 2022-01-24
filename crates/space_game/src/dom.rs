use std::future::Future;

use futures::{select, FutureExt};
use js_sys::{Function, Promise};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    BinaryType, Document, EventTarget, HtmlCanvasElement, HtmlImageElement, WebSocket, Window,
};

pub async fn content_loaded() -> Result<(), JsValue> {
    if document()?.ready_state() == "loading" {
        add_event_listener_future(window()?.as_ref(), "DOMContentLoaded")?.await;
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
        _ = add_event_listener_future(&image, "load")?.fuse() => Ok(image),
        val = add_event_listener_future(&image, "error")?.fuse() => Err(val),
    }
}

pub async fn open_websocket(url: &str) -> Result<WebSocket, JsValue> {
    let ws = WebSocket::new(url)?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    select! {
        _ = add_event_listener_future(&ws, "open")?.fuse() => Ok(ws),
        val = add_event_listener_future(&ws, "error")?.fuse() => Err(val),
    }
}

pub fn add_event_listener_future(
    target: &EventTarget,
    type_: &str,
) -> Result<impl Future<Output = JsValue>, JsValue> {
    let (cb, future) = make_callback_future();
    target.add_event_listener_with_callback(type_, &cb)?;
    Ok(future)
}

fn make_callback_future() -> (Function, impl Future<Output = JsValue>) {
    let mut resolve_opt = None;
    let future = JsFuture::from(Promise::new(&mut |resolve, _reject| {
        resolve_opt = Some(resolve);
    }));

    (resolve_opt.unwrap(), async { future.await.unwrap() })
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