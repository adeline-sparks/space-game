use js_sys::{Function, Promise};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, HtmlCanvasElement, HtmlImageElement, Window};

pub async fn dom_content_loaded() {
    if expect_document().ready_state() != "loading" {
        return;
    }

    future_from_callback(|resolve| {
        expect_window()
            .add_event_listener_with_callback("DOMContentLoaded", &resolve)
            .expect("Failed to add DOMContentLoaded event handler");
    })
    .await;
}

pub async fn animation_frame() -> f64 {
    future_from_callback(|cb| {
        expect_window()
            .request_animation_frame(&cb)
            .expect("Failed to `request_animation_frame`");
    })
    .await
    .as_f64()
    .expect("request_animation_frame did not provide a float")
        / 1e3
}

pub async fn load_image(src: &str) -> Result<HtmlImageElement, String> {
    let image = web_sys::HtmlImageElement::new().expect("Failed to create HtmlImageElement");
    image.set_src(src);
    future_from_callback(|cb| {
        image
            .add_event_listener_with_callback("load", &cb)
            .expect("Failed to register for image load event");
        image
            .add_event_listener_with_callback("error", &cb)
            .expect("Failed to register for image error event");
    })
    .await;

    if image.complete() && image.natural_height() == 0 {
        Ok(image)
    } else {
        Err("Failed to load image".to_string())
    }
}

async fn future_from_callback(mut setup: impl FnMut(Function)) -> JsValue {
    JsFuture::from(Promise::new(&mut |resolve, _reject| setup(resolve)))
        .await
        .expect("Promise did not resolve")
}

pub fn get_canvas(element_id: &str) -> Result<HtmlCanvasElement, String> {
    expect_document()
        .get_element_by_id(element_id)
        .ok_or_else(|| format!("get_element_by_id failed for `{}`", element_id))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("`{}` is not a canvas", element_id))
}

fn expect_window() -> Window {
    web_sys::window().expect("Global `window` does not exist")
}

fn expect_document() -> Document {
    expect_window()
        .document()
        .expect("Global `document` does not exist")
}
