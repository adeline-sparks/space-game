use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

pub trait WorldState: Any + Clone + Default + 'static {}

pub struct World(HashMap<TypeId, RefCell<Box<dyn Any>>>);

impl World {
    pub fn insert<S: WorldState>(&mut self, state: Box<S>) -> Option<S> {
        self.0
            .insert(TypeId::of::<S>(), RefCell::new(state))
            .map(|a| *a.into_inner().downcast().unwrap())
    }

    pub fn remove<S: WorldState>(&mut self) -> Option<Box<S>> {
        self.0
            .remove(&TypeId::of::<S>())
            .map(|a| a.into_inner().downcast().unwrap())
    }

    pub fn get<S: WorldState>(&self) -> Option<Ref<S>> {
        self.0
            .get(&TypeId::of::<S>())
            .map(|r| Ref::map(r.borrow(), |a| a.downcast_ref().unwrap()))
    }

    pub fn get_mut<S: WorldState>(&self) -> Option<RefMut<S>> {
        self.0
            .get(&TypeId::of::<S>())
            .map(|r| RefMut::map(r.borrow_mut(), |a| a.downcast_mut().unwrap()))
    }
}

pub trait WorldFnArg: Sized {
    type Builder;
    fn dependencies() -> Vec<Dependency>;
}

pub trait WorldFnArgBuilder<'a, T> {
    fn build(world: &'a World) -> T;
}

pub enum Dependency {
    Read(TypeId),
    ReadDelayed(TypeId),
    Write(TypeId),
}

pub struct Read<'s, S>(Ref<'s, S>);
pub struct ReadBuilder;

impl<'s, S: WorldState> WorldFnArg for Read<'s, S> {
    type Builder = ReadBuilder;

    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::Read(TypeId::of::<S>())]
    }
}

impl<'a, S: WorldState> WorldFnArgBuilder<'a, Read<'a, S>> for ReadBuilder {
    fn build(world: &'a World) -> Read<'a, S> {
        Read(world.get::<S>().unwrap())
    }
}

pub trait WorldFn<'w, Args> {
    type Output;

    fn call(&self, world: &'w World) -> Self::Output;
}

impl<F: Fn()> WorldFn<'_, ()> for F {
    type Output = F::Output;

    fn call(&self, _world: &World) -> Self::Output {
        (*self)()
    }
}

impl<'w, A1: WorldFnArg, F: Fn(A1)> WorldFn<'w, (A1,)> for F
where
    A1::Builder: WorldFnArgBuilder<'w, A1>,
{
    type Output = F::Output;

    fn call(&self, world: &'w World) -> Self::Output {
        (*self)(A1::Builder::build(world))
    }
}

impl<'w, A1: WorldFnArg, A2: WorldFnArg, F: Fn(A1, A2)> WorldFn<'w, (A1, A2)> for F
where
    A1::Builder: WorldFnArgBuilder<'w, A1>,
    A2::Builder: WorldFnArgBuilder<'w, A2>,
{
    type Output = F::Output;

    fn call(&self, world: &'w World) -> Self::Output {
        (*self)(A1::Builder::build(world), A2::Builder::build(world))
    }
}
