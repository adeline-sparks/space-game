use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap};
use std::ops::{Deref, DerefMut};

use super::event::EventQueue;
use super::handler::HandlerFnArg;
use super::reactor::Dependency;
use super::topic::TopicContainer;

pub trait State: Any + Clone + Default + 'static {}

pub struct StateContainer(HashMap<TypeId, RefCell<Box<dyn Any>>>);

impl StateContainer {
    pub fn insert<S: State>(&mut self, state: Box<S>) -> Option<S> {
        self.0
            .insert(TypeId::of::<S>(), RefCell::new(state))
            .map(|a| *a.into_inner().downcast().unwrap())
    }

    pub fn remove<S: State>(&mut self) -> Option<Box<S>> {
        self.0
            .remove(&TypeId::of::<S>())
            .map(|a| a.into_inner().downcast().unwrap())
    }

    pub fn get<S: State>(&self) -> Option<Ref<S>> {
        self.0
            .get(&TypeId::of::<S>())
            .map(|r| Ref::map(r.borrow(), |a| a.downcast_ref().unwrap()))
    }

    pub fn get_mut<S: State>(&self) -> Option<RefMut<S>> {
        self.0
            .get(&TypeId::of::<S>())
            .map(|r| RefMut::map(r.borrow_mut(), |a| a.downcast_mut().unwrap()))
    }
}

pub struct Reader<'s, S: State>(Ref<'s, S>);

impl<'s, S: State> HandlerFnArg<'s> for Reader<'s, S> {
    fn dependency() -> Option<Dependency> {
        Some(Dependency::ReadState(TypeId::of::<S>()))
    }

    fn build(world: &'s StateContainer, _events: &EventQueue, _topics: &TopicContainer) -> Self {
        Self(world.get().unwrap())
    }
}

impl<'s, S: State> Deref for Reader<'s, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

pub struct Writer<'s, S: State>(RefMut<'s, S>);

impl<'s, S: State> HandlerFnArg<'s> for Writer<'s, S> {
    fn dependency() -> Option<Dependency> {
        Some(Dependency::WriteState(TypeId::of::<S>()))
    }

    fn build(world: &'s StateContainer, _events: &EventQueue, _topics: &TopicContainer) -> Self {
        Self(world.get_mut().unwrap())
    }
}

impl<'s, S: State> Deref for Writer<'s, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'s, S: State> DerefMut for Writer<'s, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}