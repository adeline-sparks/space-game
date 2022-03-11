use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use anyhow::format_err;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

pub trait State: Clone + Default + 'static {
    fn id() -> StateId {
        StateId {
            id: TypeId::of::<Self>(),
            name: type_name::<Self>(),
            default_fn: || AnyState::new(Self::default()),
        }
    }
}

#[derive(Eq, Clone, Debug)]
pub struct StateId {
    id: TypeId,
    name: &'static str,
    default_fn: fn() -> AnyState,
}

impl PartialEq for StateId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for StateId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for StateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

pub struct AnyState(Box<dyn AnyStateInner>);

pub trait AnyStateInner {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn id(&self) -> StateId;
    fn clone_any(&self) -> AnyState;
}

impl<S: State + Sized> AnyStateInner for S {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn id(&self) -> StateId {
        S::id()
    }

    fn clone_any(&self) -> AnyState {
        AnyState(Box::new(self.clone()))
    }
}

impl AnyState {
    pub fn new<S: State>(s: S) -> AnyState {
        AnyState(Box::new(s))
    }

    pub fn id(&self) -> StateId {
        self.0.id()
    }

    pub fn downcast<S: State>(&self) -> Option<&S> {
        self.0.as_any().downcast_ref()
    }

    pub fn downcast_mut<S: State>(&mut self) -> Option<&mut S> {
        self.0.as_any_mut().downcast_mut()
    }
}

impl Clone for AnyState {
    fn clone(&self) -> Self {
        self.0.clone_any()
    }
}

#[derive(Default)]
pub struct StateContainer(HashMap<StateId, RefCell<AnyState>>);

impl StateContainer {
    pub fn new(ids: impl IntoIterator<Item = StateId>) -> StateContainer {
        StateContainer(
            ids.into_iter()
                .map(|id| {
                    let state = (id.default_fn)();
                    (id, RefCell::new(state))
                })
                .collect(),
        )
    }

    pub fn get<S: State>(&self) -> Option<Ref<S>> {
        let cell = self.0.get(&S::id())?;
        Some(Ref::map(cell.borrow(), |a| a.downcast::<S>().unwrap()))
    }

    pub fn get_mut<S: State>(&self) -> Option<RefMut<S>> {
        let cell = self.0.get(&S::id())?;
        Some(RefMut::map(cell.borrow_mut(), |a| {
            a.downcast_mut::<S>().unwrap()
        }))
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

    fn build(context: &'c Context) -> anyhow::Result<Reader<'c, S>> {
        let s = context
            .states
            .get()
            .ok_or_else(|| format_err!("Missing state `{}` for Reader", S::id()))?;

        Ok(Reader(s))
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

    fn build(context: &'c Context) -> anyhow::Result<DelayedReader<'c, S>> {
        let s = context
            .states
            .get()
            .ok_or_else(|| format_err!("Missing state `{}` for ReaderDelayed", S::id()))?;

        Ok(DelayedReader(s))
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

    fn build(context: &'c Context) -> anyhow::Result<Writer<'c, S>> {
        let s = context
            .states
            .get_mut()
            .ok_or_else(|| format_err!("Missing state `{}` for Writer", S::id()))?;

        Ok(Writer(s))
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
