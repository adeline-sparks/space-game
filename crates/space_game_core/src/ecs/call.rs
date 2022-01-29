use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use super::{Dependency, System, SystemId, SystemInputs, World, system::AnySystem};

pub struct CallQueue<T>(RefCell<Vec<Box<dyn FnOnce(&mut T)>>>);

impl<T> Default for CallQueue<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> CallQueue<T> {
    pub fn post(&self, func: impl FnOnce(&mut T) + 'static) {
        self.0.borrow_mut().push(Box::new(func))
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    pub fn run(&self, val: &mut T) {
        let mut vec = self.0.borrow_mut();
        while let Some(func) = vec.pop() {
            func(val);
        }
    }
}

pub struct Call<'a, T>(&'a CallQueue<T>);

impl<'a, T> Call<'a, T> {
    pub fn post(&self, func: impl FnOnce(&mut T) + 'static) {
        self.0.post(func)
    }
}

impl<'a, S: System<'a>> SystemInputs<'a> for Call<'a, S> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Call(SystemId::of::<S>()));
    }

    fn assemble(world: &'a World) -> Self {
        Call(world.call_queues.get())
    }
}

#[derive(Default)]
pub struct CallQueueMap(HashMap<SystemId, Box<dyn AnyCallQueue>>);

pub trait AnyCallQueue {
    fn run_any(&self, val: &mut dyn AnySystem);

    fn as_any(&self) -> &dyn Any;
}

impl<T: 'static> AnyCallQueue for CallQueue<T> {
    fn run_any(&self, val: &mut dyn AnySystem) {
        if !self.is_empty() {
            self.run(val.as_any_mut().downcast_mut().unwrap())
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CallQueueMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register<'a, S: System<'a>>(&mut self) {
        self.0
            .insert(SystemId::of::<S>(), Box::new(CallQueue::<S>::default()));
    }

    pub fn unregister<'a, S: System<'a>>(&mut self) {
        self.0.remove(&SystemId::of::<S>());
    }

    pub fn get<'a, S: System<'a>>(&self) -> &CallQueue<S> {
        self.0
            .get(&SystemId::of::<S>())
            .unwrap()
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    pub fn get_any(&self, id: SystemId) -> &dyn AnyCallQueue {
        self.0.get(&id).unwrap().as_ref()
    }
}
