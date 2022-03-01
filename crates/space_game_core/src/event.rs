use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};

use crate::world::World;

pub trait Event: 'static {}

pub type AnyEvent = Box<dyn Any>;

type EventHandlerFn = Box<dyn Fn(AnyEvent, &World) -> Vec<AnyEvent>>;

#[derive(Default)]
pub struct Reactor {
    event_handlers: HashMap<TypeId, EventHandlerFn>,
}

impl Reactor {
    pub fn dispatch(&self, world: &World, ev: AnyEvent) {
        let mut queue = VecDeque::new();
        queue.push_back(ev);

        while let Some(event) = queue.pop_front() {
            if let Some(handler) = self.event_handlers.get(&event.type_id()) {
                queue.extend(handler(event, world).into_iter());
            }
        }
    }
}
