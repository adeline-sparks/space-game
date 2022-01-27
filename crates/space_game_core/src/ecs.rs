use std::{any::{Any, TypeId}, collections::{HashMap, HashSet, VecDeque}, cell::RefCell};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<'a, S: System<'a>>() -> Self { Self(TypeId::of::<S>()) }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EventId(TypeId);

impl EventId {
    pub fn of<E: Event>() -> Self { Self(TypeId::of::<E>()) }
}

pub trait System<'a> : 'static {
    type Inputs: SystemInputs<'a>;

    fn update(&mut self, inputs: Self::Inputs);
    fn on_event(&mut self, _event: &dyn AnyEvent) { }
}

pub trait SystemInputs<'a> {
    fn write_dependencies(output: &mut Vec<Dependency>);
    fn assemble(systems: &'a SystemMap, events: &'a EventQueueMap) -> Self;
}

#[derive(Clone, Debug)]
pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
    Emit(EventId),
}

impl<'a, S: System<'a>> SystemInputs<'a> for &'a S {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Read(SystemId::of::<S>()));
    }

    fn assemble(systems: &'a SystemMap, _events: &'a EventQueueMap) -> Self {
        systems.get::<S>()
    }
}

#[derive(Clone, Copy)]
struct Delay<'a, S>(&'a S);

impl<'a, S: System<'a>> SystemInputs<'a> for Delay<'a, S> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::ReadDelay(SystemId::of::<S>()));
    }

    fn assemble(systems: &'a SystemMap, _events: &'a EventQueueMap) -> Self {
        Delay(systems.get::<S>())
    }
}

impl<'a, E: Event> SystemInputs<'a> for &'a EventQueue<E> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Emit(EventId::of::<E>()));
    }

    fn assemble(_systems: &'a SystemMap, events: &'a EventQueueMap) -> Self {
        events.get()
    }
}

impl<'a> SystemInputs<'a> for () {
    fn write_dependencies(_output: &mut Vec<Dependency>) { }
    fn assemble(_systems: &'a SystemMap, _events: &'a EventQueueMap) -> Self { () }
}

impl<'a, A: SystemInputs<'a>, B: SystemInputs<'a>> SystemInputs<'a> for (A, B) {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        A::write_dependencies(output);
        B::write_dependencies(output);
    }

    fn assemble(systems: &'a SystemMap, events: &'a EventQueueMap) -> Self {
        (A::assemble(systems, events), B::assemble(systems, events))
    }
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<DynAnySystem>>>,
}

trait AnySystem<'a> {
    fn dependencies(&self) -> Vec<Dependency>;
    fn any_update(&mut self, systems: &'a SystemMap, events: &'a mut EventQueueMap);
    fn any_event(&mut self, ev: &dyn AnyEvent);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

type DynAnySystem = dyn for<'a> AnySystem<'a>;

impl<'a, S: System<'a>> AnySystem<'a> for S {
    fn dependencies(&self) -> Vec<Dependency> {
        let mut result = Vec::new();
        S::Inputs::write_dependencies(&mut result);
        result
    }

    fn any_update(&mut self, systems: &'a SystemMap, events: &'a mut EventQueueMap) {
        self.update(S::Inputs::assemble(systems, events));
    }

    fn any_event(&mut self, ev: &dyn AnyEvent) {
        self.on_event(ev);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }    
}

impl From<&DynAnySystem> for SystemId {
    fn from(sys: &DynAnySystem) -> Self {
        SystemId(sys.as_any().type_id())
    }
}

impl SystemMap {
    pub fn new() -> Self { Default::default() }

    pub fn insert<S: for<'a> System<'a>>(&mut self, sys: S) {
        if self.systems.insert(SystemId::of::<S>(), Some(Box::new(sys))).is_some() {
            panic!("Can't insert duplicate system");
        }
    }
    pub fn remove<'a, S: System<'a>>(&mut self) -> S {
        *self.systems.remove(&SystemId::of::<S>())
            .expect("Can't remove system that was not inserted")
            .expect("Can't remove system that was taken")
            .as_any_box()
            .downcast()
            .unwrap()
    }

    pub fn get<'a, S: System<'a>>(&self) -> &S { 
        self.systems.get(&SystemId::of::<S>())
            .expect("Can't get system that was not inserted")
            .as_ref()
            .expect("Can't get system that was taken")
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn take_any(&mut self, id: SystemId) -> Box<DynAnySystem> { 
        self.systems.get_mut(&id)
            .expect("Can't take system that was not inserted")
            .take()
            .expect("Can't take system that was already taken")
    }

    fn untake_any(&mut self, sys: Box<DynAnySystem>) { 
        let id = SystemId::from(sys.as_ref());
        let sys_opt = self.systems.get_mut(&id)
            .expect("Can't untake system that was never inserted");
        if sys_opt.is_some() {
            panic!("Can't untake system that was never taken");
        }
        *sys_opt = Some(sys);
    }

    pub fn topological_order(&self) -> Result<Vec<SystemId>, ()> { 
        let mut dep_map = HashMap::<SystemId, Vec<SystemId>>::new();
        for sys in self.systems.values() {
            let sys = sys.as_deref().expect("Can't compute topological_order with taken System(s)");
            let sys_id = SystemId::from(sys);
            for dep in sys.dependencies() {
                match dep {
                    Dependency::Read(dep_id) => {
                        dep_map.entry(sys_id).or_default().push(dep_id);
                    }
                    Dependency::ReadDelay(dep_id) => {
                        dep_map.entry(dep_id).or_default().push(sys_id);
                    }
                    Dependency::Emit(_) => (),
                }
            }
        }
        let dep_map = dep_map;

        fn visit(id: SystemId, dep_map: &HashMap<SystemId, Vec<SystemId>>, unvisited: &mut HashSet<SystemId>, pending: &mut HashSet<SystemId>, result: &mut Vec<SystemId>) -> Result<(), ()> {
            if !unvisited.remove(&id) {
                return Ok(());
            }

            if !pending.insert(id) {
                return Err(());
            }

            if let Some(children) = dep_map.get(&id) {
                for &child in children {
                    visit(child, dep_map, unvisited, pending, result)?;
                }
            }

            pending.remove(&id);
            result.push(id);

            Ok(())
        }

        let mut unvisited = self.systems.keys().cloned().collect::<HashSet<_>>();
        let mut pending: HashSet<SystemId> = HashSet::new();
        let mut result = Vec::new();
        while let Some(&id) = unvisited.iter().next() {
            visit(id, &dep_map, &mut unvisited, &mut pending, &mut result)?;
        }

        Ok(result)
    }

    fn iter_systems_mut(&mut self) -> impl Iterator<Item=&mut DynAnySystem> {
        self.systems.values_mut().map(|slot| slot.as_mut().unwrap().as_mut())
    }
}

pub trait Event: 'static + Any { }
pub trait AnyEvent: Event {
    fn as_any(&self) -> &dyn Any;
}

impl<E: Event> AnyEvent for E {
    fn as_any(&self) -> &dyn Any { self }
}

pub struct EventQueue<E>(RefCell<VecDeque<E>>);

impl<E> Default for EventQueue<E> {
    fn default() -> Self {
        Self(Default::default())
    }
}


impl<E: Event> EventQueue<E> {
    pub fn push(&self, val: E) {
        self.0.borrow_mut().push_back(val);
    }
}

#[derive(Default)]
pub struct EventQueueMap {
    queues: HashMap<EventId, Box<dyn AnyEventQueue>>
}

trait AnyEventQueue {
    fn len(&self) -> usize;
    fn pop_any(&self) -> Option<Box<dyn AnyEvent>>;
    fn as_any(&self) -> &dyn Any;
}

impl<E: Event> AnyEventQueue for EventQueue<E> {
    fn len(&self) -> usize {
        self.0.borrow().len()
    }
    fn pop_any(&self) -> Option<Box<dyn AnyEvent>> {
        Some(Box::new(self.0.borrow_mut().pop_front()?))
    }

    fn as_any(&self) -> &dyn Any { self }
}

impl EventQueueMap {
    pub fn register<E: Event>(&mut self) {
        self.queues.insert(
            EventId::of::<E>(), 
            Box::new(EventQueue::<E>::default()));
    }

    fn get<E: Event>(&self) -> &EventQueue<E> {
        self.queues[&EventId::of::<E>()]
            .as_any()
            .downcast_ref::<EventQueue<E>>()
            .unwrap()
    }

    fn iter(&self) -> impl Iterator<Item=&dyn AnyEventQueue> {
        self.queues.values().map(|v| v.as_ref())
    }
}

#[derive(Default)]
pub struct World {
    systems: SystemMap,
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
            sys.any_update(&self.systems, &mut self.event_queues);
            self.systems.untake_any(sys);
        }

        loop {
            let mut no_events = true;
            for queue in self.event_queues.iter() {
                for _ in 0..queue.len() {
                    no_events = false;
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

            fn update(&mut self, inputs: Self::Inputs) { self.0 = inputs.0.0; }
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
