use std::{any::{Any, TypeId}, collections::VecDeque, cell::RefCell};

use super::{handler::HandlerFnArg, reactor::Dependency, state::StateContainer, topic::TopicContainer};

pub trait Event: 'static { }

pub struct AnyEvent(Box<dyn Any>);

impl AnyEvent {
    pub fn new<E: Event>(ev: E) -> Self {
        Self(Box::new(ev))
    }

    pub fn type_id(&self) -> TypeId {
        self.0.type_id()
    }

    pub fn downcast<E: Event>(&self) -> &E {
        self.0.downcast_ref().unwrap() // TODO
    }
}

#[derive(Default)]
pub struct EventQueue(RefCell<VecDeque<AnyEvent>>);

impl EventQueue {
    pub fn new() -> EventQueue {
        Default::default()
    }

    pub fn pop(&self) -> Option<AnyEvent> {
        self.0.borrow_mut().pop_front()
    }

    pub fn push(&self, ev: AnyEvent) {
        self.0.borrow_mut().push_back(ev);
    }
}

pub struct Events<'e>(&'e EventQueue);

impl<'e> Events<'e> {
    pub fn emit<E: Event>(&self, e: E) {
        self.0.push(AnyEvent::new(e));
    }
}

impl<'e> HandlerFnArg<'e> for Events<'e> {
    fn dependency() -> Option<Dependency> {
        None
    }

    fn build(_world: &'e StateContainer, events: &'e EventQueue, _topics: &'e TopicContainer) -> Self {
        Self(events)
    }
}