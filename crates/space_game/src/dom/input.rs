use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use futures::select;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{EventTarget, KeyboardEvent};

use super::{await_event, get_canvas, spawn};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Key {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
}

impl TryFrom<&KeyboardEvent> for Key {
    type Error = ();

    fn try_from(value: &KeyboardEvent) -> Result<Self, Self::Error> {
        match value.key().as_str() {
            "ArrowLeft" => Ok(Key::ArrowLeft),
            "ArrowRight" => Ok(Key::ArrowRight),
            "ArrowUp" => Ok(Key::ArrowUp),
            "ArrowDown" => Ok(Key::ArrowDown),
            _ => Err(()),
        }
    }
}

#[derive(Default)]
struct State {
    keys: HashSet<Key>,
}

pub struct InputEventListener(Rc<RefCell<State>>);

impl InputEventListener {
    pub fn from_canvas(element_id: &str) -> Result<Self, JsValue> {
        let canvas = get_canvas(element_id)?;
        canvas.set_tab_index(0);
        canvas.focus()?;
        let target = canvas.dyn_into::<EventTarget>()?;
        Ok(Self::new(target))
    }

    pub fn new(target: EventTarget) -> Self {
        let state_rc = Rc::new(RefCell::new(State::default()));
        let state_weak = Rc::downgrade(&state_rc);
        spawn(async move { listen_keyboard(&target, &state_weak).await });
        InputEventListener(state_rc)
    }

    pub fn is_key_down(&self, key: Key) -> bool {
        self.0.borrow().keys.contains(&key)
    }
}

async fn listen_keyboard(
    target: &EventTarget,
    state_weak: &Weak<RefCell<State>>,
) -> Result<(), JsValue> {
    loop {
        let ev = select! {
            ev = await_event(target, "keydown")? => ev,
            ev = await_event(target, "keyup")? => ev,
        };

        let state_rc = match state_weak.upgrade() {
            None => return Ok(()),
            Some(state_rc) => state_rc,
        };

        let ev = match ev.dyn_into::<KeyboardEvent>() {
            Err(ev) => {
                return Err(JsValue::from(format!(
                    "Failed to cast KeyboardEvent: {ev:?}"
                )))
            }
            Ok(ev) => ev,
        };

        match (ev.type_().as_str(), Key::try_from(&ev)) {
            ("keydown", Ok(key)) => {
                state_rc.borrow_mut().keys.insert(key);
            }
            ("keyup", Ok(key)) => {
                state_rc.borrow_mut().keys.remove(&key);
            }
            _ => {}
        }
    }
}
