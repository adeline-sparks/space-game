//! [`State`] and related types.

use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use anyhow::format_err;

use super::handler::{Context, Dependency, HandlerFnArg, HandlerFnArgBuilder};

/// Trait for types stored in a [`StateContainer`]
pub trait State: Clone + Default + 'static {
    /// Return the `StateId` of this type.
    fn id() -> StateId {
        StateId {
            id: TypeId::of::<Self>(),
            name: type_name::<Self>(),
            default_fn: || AnyState::new(Self::default()),
        }
    }
}

/// ID for a type which implements `State`.
#[derive(Eq, Clone, Debug)]
pub struct StateId {
    /// `TypeId` for the `State` type.
    id: TypeId,
    /// `type_name` for the `State` type.
    name: &'static str,
    /// Constructs a default value of this `State` wrapped in an `AnyState`.
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

/// Dynamically-typed container for a value that implement [`State`]
pub struct AnyState(Box<dyn AnyStateInner>);

/// Object-safe trait used inside [`AnyState`]
trait AnyStateInner {
    /// Returns `self` as an [`Any`].
    fn as_any(&self) -> &dyn Any;
    /// Returns `self` as an [`Any`] mutably.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Returns the `StateId` of `self`.
    fn id(&self) -> StateId;
    /// Clone `self` into an [`AnyState`]
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
    /// Wrap a type implementing [`State`].
    pub fn new<S: State>(s: S) -> AnyState {
        AnyState(Box::new(s))
    }

    /// Return the [`StateId`] of the underlying type.
    pub fn id(&self) -> StateId {
        self.0.id()
    }

    /// Downcast back to the inner [`Event`] type.
    pub fn downcast<S: State>(&self) -> Option<&S> {
        self.0.as_any().downcast_ref()
    }

    /// Downcast back to the inner [`Event`] type mutably.
    pub fn downcast_mut<S: State>(&mut self) -> Option<&mut S> {
        self.0.as_any_mut().downcast_mut()
    }
}

impl Clone for AnyState {
    fn clone(&self) -> Self {
        self.0.clone_any()
    }
}

/// Contains a set of types implementing [`State`].
#[derive(Default)]
pub struct StateContainer(HashMap<StateId, RefCell<AnyState>>);

impl StateContainer {
    /// Initialize from a set of `StateId`s. The `State`s are `Default` initialized.
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

    /// Get a reference to a `State` by its type.
    pub fn get<S: State>(&self) -> Option<Ref<S>> {
        let cell = self.0.get(&S::id())?;
        Some(Ref::map(cell.borrow(), |a| a.downcast::<S>().unwrap()))
    }

    /// Get a mutable reference to a `State` by its type.
    pub fn get_mut<S: State>(&self) -> Option<RefMut<S>> {
        let cell = self.0.get(&S::id())?;
        Some(RefMut::map(cell.borrow_mut(), |a| {
            a.downcast_mut::<S>().unwrap()
        }))
    }
}

/// Handler argument used to read a `State`.
pub struct Reader<'s, S: State>(Ref<'s, S>);

impl<'s, S: State> HandlerFnArg for Reader<'s, S> {
    type Builder = ReaderBuilder<S>;
    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::ReadState(S::id()));
    }
}

#[doc(hidden)]
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

/// Handler argument used to read the value of a `State`
/// on the previous cycle.
pub struct DelayedReader<'s, S: State>(Ref<'s, S>);

impl<'s, S: State> HandlerFnArg for DelayedReader<'s, S> {
    type Builder = DelayedReaderBuilder<S>;
    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::ReadStateDelayed(S::id()));
    }
}

#[doc(hidden)]
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

/// Handler argument used to write a `State`.
pub struct Writer<'s, S: State>(RefMut<'s, S>);

impl<'s, S: State> HandlerFnArg for Writer<'s, S> {
    type Builder = WriterBuilder<S>;

    fn dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::WriteState(S::id()));
    }
}

#[doc(hidden)]
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
