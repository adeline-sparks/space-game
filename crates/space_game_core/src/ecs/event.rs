use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait Event: 'static {
    fn id() -> EventId {
        EventId(TypeId::of::<Self>())
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub struct EventId(TypeId);

pub struct AnyEvent(Box<dyn Any>);

impl AnyEvent {
    pub fn new<E: Event>(ev: E) -> Self {
        Self(Box::new(ev))
    }

    pub fn id(&self) -> EventId {
        EventId(self.0.type_id())
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

pub struct EventWriter<'e>(&'e EventQueue);

impl<'e> EventWriter<'e> {
    pub fn write<E: Event>(&self, e: E) {
        self.0.push(AnyEvent::new(e));
    }
}

impl<'e> HandlerFnArg for EventWriter<'e> {
    type Builder = EventWriterBuilder;

    fn dependencies(_out: &mut Vec<Dependency>) {}
}

pub struct EventWriterBuilder;

impl<'c> HandlerFnArgBuilder<'c> for EventWriterBuilder {
    type Arg = EventWriter<'c>;

    fn build(context: &'c Context) -> EventWriter<'c> {
        EventWriter(&context.queue)
    }
}
