use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use futures::select;
use glam::{IVec2};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{EventTarget, KeyboardEvent, Element, MouseEvent, WheelEvent};

use super::{await_event, get_canvas, spawn, document};

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

struct State {
    keys: HashSet<Key>,
    mouse_pos: IVec2,
    wheel_pos: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_pos: IVec2::new(0, 0),
            wheel_pos: 0.0,
        }
    }
}

pub struct InputEventListener(Rc<RefCell<State>>);

impl InputEventListener {
    pub fn from_canvas(element_id: &str) -> Result<Self, JsValue> {
        let canvas = get_canvas(element_id)?;
        canvas.set_tab_index(0);
        canvas.focus()?;
        let target = canvas.dyn_into::<Element>()?;
        Ok(Self::new(target))
    }

    pub fn new(target: Element) -> Self {
        let state_rc = Rc::new(RefCell::new(State::default()));
        let state_weak = Rc::downgrade(&state_rc);
        let ev_target = target.dyn_ref::<EventTarget>().unwrap().clone();
        spawn(async move { listen_keyboard(&ev_target, &state_weak).await });
        let state_weak = Rc::downgrade(&state_rc);
        spawn(async move { listen_mouse(&target, &state_weak).await });
        InputEventListener(state_rc)
    }

    pub fn is_key_down(&self, key: Key) -> bool {
        self.0.borrow().keys.contains(&key)
    }

    pub fn mouse_pos(&self) -> IVec2 { 
        self.0.borrow().mouse_pos
    }

    pub fn wheel_pos(&self) -> f64 {
        self.0.borrow().wheel_pos
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
        
        ev.prevent_default();

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

async fn listen_mouse(
    target: &Element,
    state_weak: &Weak<RefCell<State>>,
) -> Result<(), JsValue> {
    let evt = target.dyn_ref::<EventTarget>().unwrap();
    
    loop {
        let ev = select! {
            ev = await_event(evt, "mousemove")? => ev,
            ev = await_event(evt, "mousedown")? => ev,
            ev = await_event(evt, "mouseup")? => ev,
            ev = await_event(evt, "wheel")? => ev,
        };

        if document()?.pointer_lock_element().as_ref() != Some(target) {
            target.request_pointer_lock();
        }

        let state_rc = match state_weak.upgrade() {
            None => return Ok(()),
            Some(state_rc) => state_rc,
        };
        
        if let Some(ev) = ev.dyn_ref::<MouseEvent>() {
            ev.prevent_default();

            let delta = IVec2::new(ev.movement_x(), -ev.movement_y());
            state_rc.borrow_mut().mouse_pos += delta;
        }

        if let Some(ev) = ev.dyn_ref::<WheelEvent>() {
            state_rc.borrow_mut().wheel_pos += ev.delta_y();
        }
    }
}
