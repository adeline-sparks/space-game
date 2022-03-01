use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};

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

#[derive(Eq, PartialEq, Hash, Clone)]
pub enum Dependency {
    Read(TypeId),
    ReadDelayed(TypeId),
    Write(TypeId),
}

impl Dependency {
    pub fn merge(deps: Vec<Dependency>) -> Vec<Dependency> {
        let mut result = deps.into_iter().collect::<HashSet<_>>();
        let writes = result
            .iter()
            .filter_map(|d| match d {
                &Dependency::Write(tid) => Some(tid),
                _ => None,
            })
            .collect::<Vec<_>>();

        for tid in writes {
            result.remove(&Dependency::Read(tid));
            result.remove(&Dependency::ReadDelayed(tid));
        }

        result.into_iter().collect::<Vec<_>>()
    }

    pub fn order(all_deps: Vec<Vec<Dependency>>) -> Vec<usize> {
        let writer = {
            let mut result = HashMap::new();
            for (idx, deps) in all_deps.iter().enumerate() {
                for dep in deps {
                    if let Dependency::Write(write_id) = dep {
                        if let Some(_conflict) = result.insert(*write_id, idx) {
                            todo!();
                        }
                    }
                }
            }
            result
        };

        let children = {
            let mut result = HashMap::new();
            for (idx, deps) in all_deps.iter().enumerate() {
                for dep in deps {
                    let (parent, child) = match dep {
                        Dependency::Read(tid) => (idx, *writer.get(&tid).unwrap()),
                        Dependency::ReadDelayed(tid) | Dependency::Write(tid) => {
                            (*writer.get(&tid).unwrap(), idx)
                        }
                    };

                    result.entry(parent).or_insert(Vec::new()).push(child);
                }
            }
            result
        };

        struct Env<'s> {
            children: &'s HashMap<usize, Vec<usize>>,
            unvisited: HashSet<usize>,
            pending: HashSet<usize>,
            result: Vec<usize>,
        }

        impl Env<'_> {
            fn visit(&mut self, idx: usize) {
                if !self.unvisited.remove(&idx) {
                    return;
                }

                self.pending.insert(idx);
                for &child_idx in self.children.get(&idx).unwrap() {
                    if self.pending.contains(&child_idx) {
                        todo!();
                    }
                    self.visit(child_idx);
                }
                self.pending.remove(&idx);

                self.result.push(idx);
            }
        }

        let mut state = Env {
            children: &children,
            unvisited: (0..all_deps.len()).into_iter().collect(),
            pending: HashSet::new(),
            result: Vec::new(),
        };

        while let Some(&idx) = state.unvisited.iter().next() {
            state.visit(idx);
        }
        state.result
    }
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

pub struct ReadDelayed<'s, S>(Ref<'s, S>);
pub struct ReadDelayedBuilder;

impl<'s, S: WorldState> WorldFnArg for ReadDelayed<'s, S> {
    type Builder = ReadDelayedBuilder;

    fn dependencies() -> Vec<Dependency> {
        vec![Dependency::Read(TypeId::of::<S>())]
    }
}

impl<'a, S: WorldState> WorldFnArgBuilder<'a, ReadDelayed<'a, S>> for ReadDelayedBuilder {
    fn build(world: &'a World) -> ReadDelayed<'a, S> {
        ReadDelayed(world.get::<S>().unwrap())
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

pub struct AnyWorldFn<R> {
    dependencies: Vec<Dependency>,
    call_fn: Box<dyn Fn(&World) -> R>,
}

impl<R> AnyWorldFn<R> {
    pub fn new<A: WorldFnArg, F: for<'w> WorldFn<'w, A, Output = R> + 'static>(f: F) -> Self {
        AnyWorldFn {
            dependencies: A::dependencies(),
            call_fn: Box::new(move |world| f.call(world)),
        }
    }

    pub fn dependencies(&self) -> &[Dependency] {
        self.dependencies.as_slice()
    }

    pub fn call(&self, world: &World) -> R {
        (self.call_fn)(world)
    }
}
