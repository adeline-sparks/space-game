mod system;
pub use self::system::{Delay, Dependency, System, SystemId, SystemInputs, SystemMap};

mod call;
pub use self::call::{Call, CallQueueMap};

#[derive(Default)]
pub struct World {
    systems: SystemMap,
    call_queues: CallQueueMap,
    topological_order: Option<Vec<SystemId>>,
}

impl World {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert<S: for<'a> System<'a>>(&mut self, sys: S) {
        self.topological_order = None;
        self.call_queues.register::<S>();
        self.systems.insert(sys);
    }

    pub fn remove<'a, S: System<'a>>(&mut self) -> S {
        self.topological_order = None;
        self.call_queues.unregister::<S>();
        self.systems.remove()
    }

    pub fn get<'a, S: System<'a>>(&self) -> &S {
        self.systems.get()
    }

    pub fn get_mut<'a, S: System<'a>>(&mut self) -> &mut S {
        self.systems.get_mut()
    }

    pub fn update(&mut self) {
        self.topological_order
            .get_or_insert_with(|| self.systems.topological_order().unwrap());
        let order = self.topological_order.as_ref().unwrap().as_slice();

        for &id in order {
            let mut sys = self.systems.take_any(id);
            self.call_queues.get_any(id).run_any(sys.as_any_mut());
            sys.any_update(&self);
            self.systems.untake_any(sys);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple() {
        struct SysA(u32);
        struct SysB(u32);
        struct SysC(u32);

        #[derive(Clone, Default, Debug, Eq, PartialEq)]
        struct Ev(u32);

        impl<'a> System<'a> for SysA {
            type Inputs = Delay<'a, SysC>;

            fn update(&mut self, inputs: Self::Inputs) {
                self.0 = inputs.0;
            }
        }

        impl<'a> System<'a> for SysB {
            type Inputs = ();

            fn update(&mut self, _inputs: Self::Inputs) {
                self.0 = self.0 + 2;
            }
        }

        impl<'a> System<'a> for SysC {
            type Inputs = (&'a SysA, &'a SysB);

            fn update(&mut self, inputs: Self::Inputs) {
                self.0 = inputs.0 .0 + inputs.1 .0;
            }
        }

        let mut world = World::new();
        world.insert(SysA(0));
        world.insert(SysB(0));
        world.insert(SysC(0));

        let mut val = 0;
        for i in 0..10 {
            world.update();
            val += 2 * i;
            assert_eq!(world.get::<SysC>().0, val);
        }
    }
}
