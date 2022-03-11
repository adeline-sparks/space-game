//! [`Event`] and related types.

use std::any::{type_name, Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{self, Debug, Display};
use std::hash::Hash;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

/// Trait for types which can be dispatched via the [`Reactor`].
pub trait Event: Debug + 'static {
    /// Return the `EventId` for this type.
    fn id() -> EventId {
        EventId {
            id: TypeId::of::<Self>(),
            name: type_name::<Self>(),
        }
    }
}

/// Identifier of a type which implements [`Event`]
#[derive(Eq, Clone, Debug)]
pub struct EventId {
    /// `TypeId` for the `Event` type.
    id: TypeId,
    /// `type_name` for `Event` type.
    name: &'static str,
}

impl PartialEq for EventId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for EventId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// Dynamically-typed container for a value that implement [`Event`]
pub struct AnyEvent(Box<dyn AnyEventInner>);

/// Object-safe trait used inside [`AnyEvent`]
trait AnyEventInner {
    /// Returns `self` as an [`Any`]
    fn as_any(&self) -> &dyn Any;
    /// Return the [`EventId`] of `self`.
    fn id(&self) -> EventId;
    /// Calls [`Debug::debug`] on `self`
    fn debug_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

impl<E: Event + Sized> AnyEventInner for E {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn id(&self) -> EventId {
        E::id()
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl AnyEvent {
    /// Wrap a type implementing [`Event`].
    pub fn new<E: Event>(ev: E) -> Self {
        Self(Box::new(ev))
    }

    /// Return the [`EventId`] of the underlying type.
    pub fn id(&self) -> EventId {
        self.0.id()
    }

    /// Downcast back to the inner [`Event`] type.
    pub fn downcast<E: Event>(&self) -> Option<&E> {
        self.0.as_any().downcast_ref()
    }
}

impl Debug for AnyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug_fmt(f)
    }
}

/// Interior-mutability queue used to store pending events.
#[derive(Default)]
pub struct EventQueue(RefCell<VecDeque<AnyEvent>>);

impl EventQueue {
    /// Construct an empty queue.
    pub fn new() -> EventQueue {
        Default::default()
    }

    /// Pop from the front of the queue.
    pub fn pop(&self) -> Option<AnyEvent> {
        self.0.borrow_mut().pop_front()
    }

    /// Push to the back of the queue.
    pub fn push(&self, ev: AnyEvent) {
        self.0.borrow_mut().push_back(ev);
    }
}

/// Handler argument used to write events.
pub struct EventWriter<'e>(&'e EventQueue);

impl<'e> EventWriter<'e> {
    /// Write an event.
    pub fn write<E: Event>(&self, e: E) {
        self.0.push(AnyEvent::new(e));
    }
}

impl<'e> HandlerFnArg for EventWriter<'e> {
    type Builder = EventWriterBuilder;

    fn dependencies(_out: &mut Vec<Dependency>) {}
}

#[doc(hidden)]
pub struct EventWriterBuilder;

impl<'c> HandlerFnArgBuilder<'c> for EventWriterBuilder {
    type Arg = EventWriter<'c>;

    fn build(context: &'c Context) -> anyhow::Result<EventWriter<'c>> {
        Ok(EventWriter(context.queue))
    }
}
