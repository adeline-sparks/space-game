use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait State: Clone + 'static {
    fn id() -> StateId {
        StateId(TypeId::of::<Self>())
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub struct StateId(TypeId);

#[derive(Default)]
pub struct StateContainer(HashMap<StateId, RefCell<Box<dyn Any>>>);

impl StateContainer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert<S: State>(&mut self, state: Box<S>) -> Option<S> {
        self.0
            .insert(S::id(), RefCell::new(state))
            .map(|a| *a.into_inner().downcast().unwrap())
    }

    pub fn insert_default<S: State + Default>(&mut self) -> Option<S> {
        self.insert(Box::new(S::default()))
    }

    pub fn remove<S: State>(&mut self) -> Option<Box<S>> {
        self.0
            .remove(&S::id())
            .map(|a| a.into_inner().downcast().unwrap())
    }

    pub fn get<S: State>(&self) -> Option<Ref<S>> {
        self.0
            .get(&S::id())
            .map(|r| Ref::map(r.borrow(), |a| a.downcast_ref().unwrap()))
    }

    pub fn get_mut<S: State>(&self) -> Option<RefMut<S>> {
        self.0
            .get(&S::id())
            .map(|r| RefMut::map(r.borrow_mut(), |a| a.downcast_mut().unwrap()))
    }
}

pub struct Reader<'s, S: State>(Ref<'s, S>);

impl<'s, S: State> HandlerFnArg for Reader<'s, S> {
    type Builder = ReaderBuilder<S>;
    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::ReadState(S::id()));
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
    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::ReadStateDelayed(S::id()));
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

    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::WriteState(S::id()));
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
