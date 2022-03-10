use std::any::{type_name, Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{self, Debug, Formatter};

use super::dependency::Dependency;
use super::handler::{Context, HandlerFnArg, HandlerFnArgBuilder};

pub trait Event: Debug + 'static {
    fn id() -> EventId {
        EventId(TypeId::of::<Self>())
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct EventId(TypeId);

pub struct AnyEvent(Box<dyn AnyEventInner>);

pub trait AnyEventInner {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
    fn debug_fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error>;
}

impl<E: Event + Sized> AnyEventInner for E {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        type_name::<E>()
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Debug::fmt(self, f)
    }
}

impl AnyEvent {
    pub fn new<E: Event>(ev: E) -> Self {
        Self(Box::new(ev))
    }

    pub fn id(&self) -> EventId {
        EventId(self.0.as_any().type_id())
    }

    pub fn type_name(&self) -> &'static str {
        self.0.type_name()
    }

    #[track_caller]
    pub fn downcast<E: Event>(&self) -> Option<&E> {
        self.0.as_any().downcast_ref()
    }
}

impl Debug for AnyEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.debug_fmt(f)
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

    fn build(context: &'c Context) -> anyhow::Result<EventWriter<'c>> {
        Ok(EventWriter(&context.queue))
    }
}
