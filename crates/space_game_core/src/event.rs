use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};

use crate::world::World;

pub trait Event: 'static {}

pub type AnyEvent = Box<dyn Any>;

pub struct Dispatcher(HashMap<TypeId, Box<dyn Fn(&World, AnyEvent) -> Vec<AnyEvent>>>);

impl Dispatcher {
    pub fn install_handler<E: Event>(&mut self, f: impl Fn(&World, E) -> Vec<AnyEvent> + 'static) {
        self.0.insert(
            TypeId::of::<E>(),
            Box::new(move |world, ev| f(world, *ev.downcast().unwrap())),
        );
    }

    pub fn dispatch(&self, world: &World, ev: AnyEvent) {
        let mut queue = VecDeque::new();
        queue.push_back(ev);

        while let Some(ev) = queue.pop_front() {
            if let Some(handler) = self.0.get(&ev.type_id()) {
                queue.extend(handler(world, ev).into_iter());
            }
        }
    }
}
