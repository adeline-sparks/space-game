use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use nalgebra::Vector2;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AddEventListenerOptions, Element, Event, KeyboardEvent, MouseEvent, WheelEvent};

use super::{document, get_canvas, DomError};

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Key(Cow<'static, str>);

pub mod key_consts {
    use std::borrow::Cow;

    use super::Key;

    pub const ARROW_LEFT: Key = Key(Cow::Borrowed("ArrowLeft"));
    pub const ARROW_RIGHT: Key = Key(Cow::Borrowed("ArrowRight"));
    pub const ARROW_UP: Key = Key(Cow::Borrowed("ArrowUp"));
    pub const ARROW_DOWN: Key = Key(Cow::Borrowed("ArrowDown"));
}

impl TryFrom<&KeyboardEvent> for Key {
    type Error = ();

    fn try_from(value: &KeyboardEvent) -> Result<Self, Self::Error> {
        Ok(Key(Cow::from(value.key())))
    }
}

impl Key {
    pub fn ch(ch: char) -> Key {
        Key(Cow::Owned(ch.to_string()))
    }
}

struct State {
    keys: HashSet<Key>,
    mouse_pos: Vector2<i32>,
    wheel_pos: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_pos: Vector2::zeros(),
            wheel_pos: 0.0,
        }
    }
}

impl State {
    fn apply_event(&mut self, ev: &Event) {
        if let Some(ev) = ev.dyn_ref::<MouseEvent>() {
            self.mouse_pos += Vector2::new(ev.movement_x(), ev.movement_y());

            if let Some(ev) = ev.dyn_ref::<WheelEvent>() {
                self.wheel_pos += ev.delta_y();
            }
        } else if let Some(ev) = ev.dyn_ref::<KeyboardEvent>() {
            match (ev.type_().as_str(), Key::try_from(ev)) {
                ("keydown", Ok(key)) => {
                    self.keys.insert(key);
                }
                ("keyup", Ok(key)) => {
                    self.keys.remove(&key);
                }
                _ => {}
            }
        }
    }
}

pub struct InputEventListener {
    state: Rc<RefCell<State>>,
    target: Element,
    listener: Closure<dyn FnMut(JsValue) -> Result<(), JsValue>>,
}

const EVENT_TYPES: &[&str] = &[
    "keydown",
    "keyup",
    "mousemove",
    "mousedown",
    "mouseup",
    "wheel",
];

impl InputEventListener {
    pub fn from_canvas(element_id: &str) -> Result<Self, DomError> {
        let canvas = get_canvas(element_id)?;
        canvas.set_tab_index(0);
        canvas.focus()?;
        let target = canvas.unchecked_into();
        Self::new(target)
    }

    pub fn new(target: Element) -> Result<Self, DomError> {
        let state = Rc::new(RefCell::new(State::default()));

        let listener: Closure<dyn FnMut(JsValue) -> Result<(), JsValue>> = {
            let target = target.clone();
            let state = state.clone();
            let document = document()?;

            Closure::wrap(Box::new(move |ev: JsValue| {
                let ev = ev.unchecked_ref::<Event>();
                if document.pointer_lock_element().as_ref() == Some(&target) {
                    if ev.type_() != "wheel" {
                        ev.prevent_default();
                    }
                    state.borrow_mut().apply_event(ev);
                } else if ev.type_() == "mousedown" {
                    target.request_pointer_lock();
                }

                Ok(())
            }))
        };

        for &type_ in EVENT_TYPES {
            let passive = type_ == "wheel";
            target.add_event_listener_with_callback_and_add_event_listener_options(
                type_,
                listener.as_ref().unchecked_ref(),
                AddEventListenerOptions::new().passive(passive),
            )?;
        }

        Ok(InputEventListener {
            state,
            target,
            listener,
        })
    }

    pub fn is_key_down(&self, key: &Key) -> bool {
        self.state.borrow().keys.contains(key)
    }

    pub fn mouse_pos(&self) -> Vector2<i32> {
        self.state.borrow().mouse_pos
    }

    pub fn wheel_pos(&self) -> f64 {
        self.state.borrow().wheel_pos
    }
}

impl Drop for InputEventListener {
    fn drop(&mut self) {
        for type_ in EVENT_TYPES {
            // TODO log
            let _ = self
                .target
                .remove_event_listener_with_callback(type_, self.listener.as_ref().unchecked_ref());
        }
    }
}
