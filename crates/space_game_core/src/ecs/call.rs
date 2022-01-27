use std::{any::{TypeId, Any}, collections::HashMap, cell::RefCell};

use super::{System, SystemInputs, Dependency, SystemId, World};

pub struct CallQueue<T>(RefCell<Vec<Box<dyn FnOnce(&mut T)>>>);

impl<T> Default for CallQueue<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> CallQueue<T> {
    pub fn post<F: FnOnce(&mut T) + 'static>(&self, func: F) {
        self.0.borrow_mut().push(Box::new(func))
    }

    pub fn run(&self, val: &mut T) {
        let len = self.0.borrow().len();
        for func in self.0.borrow_mut().drain(0..len) {
            func(val);
        }
    }
}

pub struct Call<'a, S>(&'a CallQueue<S>);

impl<'a, S: System<'a>> SystemInputs<'a> for Call<'a, S> {
    fn write_dependencies(output: &mut Vec<super::Dependency>) {
        output.push(Dependency::Call(SystemId::of::<S>()));
    }

    fn assemble(world: &'a World) -> Self {
        Call(world.call_queues.get())
    }
}

#[derive(Default)]
pub struct CallQueueMap(HashMap<TypeId, Box<dyn AnyCallQueue>>);

pub trait AnyCallQueue {
    fn run_any(&self, val: &mut dyn Any);

    fn as_any(&self) -> &dyn Any;
}

impl<T: 'static> AnyCallQueue for CallQueue<T> {
    fn run_any(&self, val: &mut dyn Any) {
        self.run(val.downcast_mut().unwrap())
    }

    fn as_any(&self) -> &dyn Any { self }
}

impl CallQueueMap {
    pub fn new() -> Self { Default::default() }

    pub fn register<T: 'static>(&mut self) {
        self.0.insert(TypeId::of::<T>(), Box::new(CallQueue::<T>::default()));
    }

    pub fn get<T: 'static>(&self) -> &CallQueue<T> {
        self.0.get(&TypeId::of::<T>()).unwrap().as_any().downcast_ref().unwrap()
    }

    pub fn get_any(&self, id: TypeId) -> &dyn AnyCallQueue {
        self.0.get(&id).unwrap().as_ref()
    }
}