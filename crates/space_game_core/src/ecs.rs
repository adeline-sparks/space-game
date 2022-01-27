mod system;
pub use self::system::{SystemMap, SystemId, System, SystemInputs, Dependency, Delay};

mod event;
pub use self::event::{AnyEvent, Event, EventId, EventQueue, EventQueueMap};

mod call;
pub use self::call::CallQueueMap;

#[derive(Default)]
pub struct World {
    systems: SystemMap,
    call_queues: CallQueueMap,
    event_queues: EventQueueMap,
    topological_order: Option<Vec<SystemId>>,
}

impl World {
    pub fn new() -> Self { Default::default() }

    pub fn systems(&self) -> &SystemMap { 
        &self.systems 
    }

    pub fn systems_mut(&mut self) -> &mut SystemMap { 
        self.topological_order = None;
        &mut self.systems
    }

    pub fn events(&self) -> &EventQueueMap { 
        &self.event_queues
    }

    pub fn events_mut(&mut self) -> &mut EventQueueMap { 
        &mut self.event_queues
    }

    pub fn update(&mut self) {
        let order = self.topological_order
            .get_or_insert_with(|| self.systems.topological_order().unwrap())
            .as_slice();

        for &id in order {
            let mut sys = self.systems.take_any(id);
            self.call_queues.get_any(id.type_id()).run_any(sys.as_any_mut());
            sys.any_update(&self.systems, &self.event_queues, &self.call_queues); // TODO just pass &World ?
            self.systems.untake_any(sys);
        }

        loop {
            let mut no_events = true;
            for queue in self.event_queues.iter() {
                let len = queue.len();
                if len == 0 {
                    continue;
                }

                no_events = false;
                for _ in 0..len {
                    let ev = queue.pop_any().unwrap();
                    for sys in self.systems.iter_systems_mut() {
                        sys.any_event(ev.as_ref());
                    }
                }
            }

            if no_events {
                break;
            }
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

            fn update(&mut self, inputs: Self::Inputs) { self.0 = inputs.0; }
        }

        impl<'a> System<'a> for SysB {
            type Inputs = &'a EventQueue<Ev>;

            fn update(&mut self, inputs: Self::Inputs) {
                inputs.push(Ev(self.0 + 2));
            }

            fn on_event(&mut self, event: &dyn AnyEvent) {
                let e: &Ev = event.as_any().downcast_ref().unwrap();
                self.0 = e.0;
            }
        }

        impl<'a> System<'a> for SysC {
            type Inputs = (&'a SysA, &'a SysB);

            fn update(&mut self, inputs: Self::Inputs) { 
                self.0 = inputs.0.0 + inputs.1.0;
            }
        }

        impl Event for Ev { }

        let mut world = World::new();
        world.systems_mut().insert(SysA(0));
        world.systems_mut().insert(SysB(0));
        world.systems_mut().insert(SysC(0));
        world.events_mut().register::<Ev>();

        let mut val = 0;
        for i in 0..10 {
            world.update();
            val += 2*i;
            assert_eq!(world.systems().get::<SysC>().0, val);
        }
    }
}
