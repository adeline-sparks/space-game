use std::{any::{TypeId, Any}, collections::{VecDeque, HashMap}, cell::RefCell};

use super::{Dependency, SystemInputs, World};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EventId(TypeId);

impl EventId {
    pub fn of<E: Event>() -> Self { Self(TypeId::of::<E>()) }
}

pub trait Event: 'static + Any { }
pub trait AnyEvent: Event {
    fn as_any(&self) -> &dyn Any;
}

impl<E: Event> AnyEvent for E {
    fn as_any(&self) -> &dyn Any { self }
}

pub struct EventQueue<E>(RefCell<VecDeque<E>>);

impl<E> Default for EventQueue<E> {
    fn default() -> Self {
        Self(Default::default())
    }
}


impl<E: Event> EventQueue<E> {
    pub fn push(&self, val: E) {
        self.0.borrow_mut().push_back(val);
    }

    pub fn pop(&self) -> Option<E> {
        self.0.borrow_mut().pop_front()
    }
}

impl<'a, E: Event> SystemInputs<'a> for &'a EventQueue<E> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Emit(EventId::of::<E>()));
    }

    fn assemble(world: &'a World) -> Self {
        world.events().get()
    }
}

#[derive(Default)]
pub struct EventQueueMap {
    queues: HashMap<EventId, Box<dyn AnyEventQueue>>
}

pub trait AnyEventQueue {
    fn len(&self) -> usize;
    fn pop_any(&self) -> Option<Box<dyn AnyEvent>>;
    fn as_any(&self) -> &dyn Any;
}

impl<E: Event> AnyEventQueue for EventQueue<E> {
    fn len(&self) -> usize {
        self.0.borrow().len()
    }
    fn pop_any(&self) -> Option<Box<dyn AnyEvent>> {
        Some(Box::new(self.0.borrow_mut().pop_front()?))
    }

    fn as_any(&self) -> &dyn Any { self }
}

impl EventQueueMap {
    pub fn register<E: Event>(&mut self) {
        self.queues.insert(
            EventId::of::<E>(), 
            Box::new(EventQueue::<E>::default()));
    }

    pub fn get<E: Event>(&self) -> &EventQueue<E> {
        self.queues[&EventId::of::<E>()]
            .as_any()
            .downcast_ref::<EventQueue<E>>()
            .unwrap()
    }

    pub fn iter(&self) -> impl Iterator<Item=&dyn AnyEventQueue> {
        self.queues.values().map(|v| v.as_ref())
    }
}