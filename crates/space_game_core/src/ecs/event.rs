use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;

use super::{Dependency, SystemInputs, World};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EventId(TypeId);

impl EventId {
    pub fn of<E: Event>() -> Self {
        Self(TypeId::of::<E>())
    }
}

pub trait Event: 'static + Any {}

pub struct AnyEvent(Box<dyn Any>);

impl AnyEvent {
    pub fn handler<E: Event>(&self, mut func: impl FnMut(&E)) -> &Self {
        if let Some(ev) = self.0.downcast_ref() {
            func(ev);
        }
        self
    }
}

impl From<Box<dyn Any>> for AnyEvent {
    fn from(ev: Box<dyn Any>) -> Self {
        Self(ev)
    }
}

pub struct Emit<'a, E> {
    queue: &'a EventQueue,
    _phantom: PhantomData<&'a E>,
}

impl<'a, E: Event> Emit<'a, E> {
    pub fn emit(&self, val: E) {
        self.queue.push(Box::new(val));
    }
}

impl<'a, E: Event> SystemInputs<'a> for Emit<'a, E> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Emit(EventId::of::<E>()));
    }

    fn assemble(world: &'a World) -> Self {
        Emit {
            queue: &world.event_queue,
            _phantom: PhantomData,
        }
    }
}

#[derive(Default)]
pub struct EventQueue(RefCell<VecDeque<Box<dyn Any>>>);

impl EventQueue {
    pub fn push(&self, ev: Box<dyn Any>) {
        self.0.borrow_mut().push_back(ev);
    }

    pub fn pop(&self) -> Option<Box<dyn Any>> {
        self.0.borrow_mut().pop_front()
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
}
