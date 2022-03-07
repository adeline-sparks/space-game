use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait State: Any + Clone + 'static {}

#[derive(Default)]
pub struct StateContainer(HashMap<TypeId, RefCell<Box<dyn Any>>>);

impl StateContainer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert<S: State>(&mut self, state: Box<S>) -> Option<S> {
        self.0
            .insert(TypeId::of::<S>(), RefCell::new(state))
            .map(|a| *a.into_inner().downcast().unwrap())
    }

    pub fn insert_default<S: State + Default>(&mut self) -> Option<S> {
        self.insert(Box::new(S::default()))
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

impl<'s, S: State> HandlerFnArg for Reader<'s, S> {
    type Builder = ReaderBuilder<S>;
    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::ReadState(TypeId::of::<S>())]
    }
}

pub struct ReaderBuilder<S>(PhantomData<S>);

impl<'c, S: State> HandlerFnArgBuilder<'c> for ReaderBuilder<S> {
    type Arg = Reader<'c, S>;

    fn build(context: &'c Context) -> Reader<'c, S> {
        Reader(context.states.get().unwrap())
    }
}

impl<'s, S: State> Deref for Reader<'s, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

pub struct DelayedReader<'s, S: State>(Ref<'s, S>);

impl<'s, S: State> HandlerFnArg for DelayedReader<'s, S> {
    type Builder = DelayedReaderBuilder<S>;
    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::ReadStateDelayed(TypeId::of::<S>())]
    }
}

pub struct DelayedReaderBuilder<S>(PhantomData<S>);

impl<'c, S: State> HandlerFnArgBuilder<'c> for DelayedReaderBuilder<S> {
    type Arg = DelayedReader<'c, S>;

    fn build(context: &'c Context) -> DelayedReader<'c, S> {
        DelayedReader(context.states.get().unwrap())
    }
}

impl<'s, S: State> Deref for DelayedReader<'s, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

pub struct Writer<'s, S: State>(RefMut<'s, S>);

impl<'s, S: State> HandlerFnArg for Writer<'s, S> {
    type Builder = WriterBuilder<S>;

    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::WriteState(TypeId::of::<S>())]
    }
}

pub struct WriterBuilder<S>(PhantomData<S>);

impl<'c, S: State> HandlerFnArgBuilder<'c> for WriterBuilder<S> {
    type Arg = Writer<'c, S>;

    fn build(context: &'c Context) -> Writer<'c, S> {
        Writer(context.states.get_mut().unwrap())
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
