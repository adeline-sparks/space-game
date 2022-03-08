use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

#[derive(Eq, Clone, Debug)]
pub struct StateId {
    id: TypeId,
    name: &'static str,
    default_fn: fn() -> Box<dyn AnyState>,
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

pub trait State: Clone + Default + 'static {
    fn id() -> StateId {
        StateId {
            id: TypeId::of::<Self>(),
            name: type_name::<Self>(),
            default_fn: || Box::new(Self::default()),
        }
    }
}

#[derive(Default)]
pub struct StateContainer(HashMap<StateId, RefCell<Box<dyn AnyState>>>);

pub trait AnyState: Any + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn AnyState>;
}

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

    pub fn get<S: State>(&self) -> Ref<S> {
        Ref::map(self.0[&S::id()].borrow(), |a| {
            a.as_any().downcast_ref::<S>().unwrap()
        })
    }

    pub fn get_mut<S: State>(&self) -> RefMut<S> {
        RefMut::map(self.0[&S::id()].borrow_mut(), |a| {
            a.as_any_mut().downcast_mut::<S>().unwrap()
        })
    }
}

impl Clone for StateContainer {
    fn clone(&self) -> Self {
        StateContainer(
            self.0
                .iter()
                .map(|(id, r)| (id.clone(), RefCell::new(r.borrow().clone_box())))
                .collect(),
        )
    }
}

impl<S: Any + State> AnyState for S {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn AnyState> {
        Box::new(self.clone())
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
        Reader(context.states.get())
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
        DelayedReader(context.states.get())
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
        Writer(context.states.get_mut())
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
