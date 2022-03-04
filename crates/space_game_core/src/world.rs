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

#[derive(Eq, PartialEq, Hash, Clone)]
pub enum Dependency {
    Read(TypeId),
    ReadDelayed(TypeId),
    Write(TypeId),
}

impl Dependency {
    pub fn execution_order(all_deps: &[&[Dependency]]) -> Vec<usize> {
        let writer = {
            let mut result = HashMap::new();
            for (idx, &deps) in all_deps.iter().enumerate() {
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
            for (idx, &deps) in all_deps.iter().enumerate() {
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
