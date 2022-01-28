use std::{any::{TypeId, Any}, collections::{VecDeque}, cell::{RefCell}, marker::PhantomData};

use super::{Dependency, SystemInputs, World};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EventId(TypeId);

impl EventId {
    pub fn of<E: Event>() -> Self { Self(TypeId::of::<E>()) }
}

pub trait Event: 'static + Any { }
pub trait AnyEvent: Event {
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
}

impl<E: Event> AnyEvent for E {
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> { self }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct EventQueue<'a, E> {
    storage: &'a RefCell<VecDeque<Box<dyn AnyEvent>>>,
    _phantom: PhantomData<&'a E>,
}

impl<'a, E: Event> EventQueue<'a, E> {
    pub fn push(&self, val: E) {
        self.storage.borrow_mut().push_back(Box::new(val));
    }

    pub fn pop(&self) -> Option<E> {
        self.storage.borrow_mut().pop_front().map(|x| *x.as_any_box().downcast::<E>().unwrap())
    }
}

impl<'a, E: Event> SystemInputs<'a> for EventQueue<'a, E> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Emit(EventId::of::<E>()));
    }

    fn assemble(world: &'a World) -> Self {
        world.event_queues.get::<E>()
    }
}

#[derive(Default)]
pub struct EventQueueMap(RefCell<VecDeque<Box<dyn AnyEvent>>>);

impl EventQueueMap {
    pub fn get<E: Event>(&self) -> EventQueue<'_, E> {
        EventQueue { storage: &self.0, _phantom: PhantomData }
    }

    pub fn pop_any(&self) -> Option<Box<dyn AnyEvent>> { self.0.borrow_mut().pop_front() }

    pub fn len(&self) -> usize { self.0.borrow().len() }
}