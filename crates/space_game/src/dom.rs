use futures::future::FusedFuture;
use futures::{select, Future, FutureExt};
use js_sys::{Function, Promise};

use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    console, AddEventListenerOptions, BinaryType, Document, EventTarget, HtmlCanvasElement,
    HtmlImageElement, WebSocket, Window, 
};

mod input;
pub use input::{key_consts, InputEventListener, Key};

#[derive(Error, Debug)]
pub enum DomError {
    #[error("Element `{0}` not found")]
    ElementNotFound(String),
    #[error("Global `window.document` missing")]
    DocumentMissing,
    #[error("Global `window` missing")]
    WindowMissing,
    #[error("Global `location` missing")]
    LocationMissing,
    #[error("Base URI missing")]
    BaseUriMissing,
    #[error("Unknown protocol")]
    UnknownProtocol,
    #[error("Caught exception")]
    CaughtException,
    #[error("Image load failed")]
    ImageError,
    #[error("WebSocket connection failed")]
    WebSocketError,
}

impl From<JsValue> for DomError {
    fn from(value: JsValue) -> Self {
        console::error_2(&"Rust caught exception".into(), &value);
        DomError::CaughtException
    }
}

pub async fn content_loaded() -> Result<(), DomError> {
    if document()?.ready_state() == "loading" {
        await_event(window()?.as_ref(), "DOMContentLoaded")?.await;
    }

    Ok(())
}

pub async fn animation_frame() -> Result<f64, DomError> {
    let (cb, future) = make_callback_future();
    window()?.request_animation_frame(&cb)?;

    Ok(future.await.unchecked_into_f64())
}

pub async fn load_image(src: &str) -> Result<HtmlImageElement, DomError> {
    let image = web_sys::HtmlImageElement::new()?;
    image.set_src(src);

    select! {
        _ = await_event(&image, "load")? => Ok(image),
        _ = await_event(&image, "error")? => Err(DomError::ImageError),
    }
}

pub async fn load_text(src: &str) -> Result<String, DomError> {
    let request = web_sys::XmlHttpRequest::new()?;
    request.open("GET", src)?;
    request.send()?;

    select! {
        _ = await_event(&request, "load")? => Ok(request.response_text()?.unwrap()),
        _ = await_event(&request, "error")? => Err(DomError::ImageError),
    }    
}

pub async fn open_websocket(rel_uri: &str) -> Result<WebSocket, DomError> {
    assert!(!rel_uri.starts_with('/'));
    assert!(!rel_uri.contains(':'));

    let mut uri = get_base_uri()?;
    if uri.starts_with("http:") {
        uri.replace_range(0..4, "ws");
    } else if uri.starts_with("https:") {
        uri.replace_range(0..5, "wss");
    }
    let base_pos = uri.rfind('/').map(|p| p + 1).unwrap_or(0);
    uri.replace_range(base_pos.., rel_uri);

    let ws = WebSocket::new(&uri)?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    select! {
        _ = await_event(&ws, "open")? => Ok(ws),
        _ = await_event(&ws, "error")? => Err(DomError::WebSocketError),
    }
}

pub fn await_event(
    target: &EventTarget,
    type_: &str,
) -> Result<impl FusedFuture<Output = JsValue>, DomError> {
    let (cb, future) = make_callback_future();
    target.add_event_listener_with_callback_and_add_event_listener_options(
        type_,
        &cb,
        AddEventListenerOptions::new().once(true),
    )?;
    Ok(future)
}

fn make_callback_future() -> (Function, impl FusedFuture<Output = JsValue>) {
    let mut resolve_opt = None;
    let future = JsFuture::from(Promise::new(&mut |resolve, _reject| {
        resolve_opt = Some(resolve);
    }));

    (resolve_opt.unwrap(), future.map(|v| v.unwrap()).fuse())
}

pub fn spawn(fut: impl Future<Output = anyhow::Result<()>> + 'static) {
    let _ = future_to_promise(async move {
        fut.await
            .map(|()| JsValue::NULL)
            .map_err(|err| JsValue::from(format!("{:?}", err)))
    });
}

pub fn get_canvas(element_id: &str) -> Result<HtmlCanvasElement, DomError> {
    Ok(document()?
        .get_element_by_id(element_id)
        .ok_or_else(|| DomError::ElementNotFound(element_id.into()))?
        .unchecked_into::<web_sys::HtmlCanvasElement>())
}

pub fn get_base_uri() -> Result<String, DomError> {
    document()?
        .base_uri()?
        .ok_or(DomError::BaseUriMissing)
}

fn window() -> Result<Window, DomError> {
    web_sys::window().ok_or(DomError::WindowMissing)
}

fn document() -> Result<Document, DomError> {
    window()?.document().ok_or(DomError::DocumentMissing)
}
