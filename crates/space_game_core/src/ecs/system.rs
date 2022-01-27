use std::{any::{TypeId, Any}, collections::{HashMap, HashSet}, ops::Deref};

use super::{AnyEvent, EventQueueMap, EventId, CallQueueMap};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<'a, S: System<'a>>() -> Self { Self(TypeId::of::<S>()) }

    pub fn type_id(self) -> TypeId { self.0 }
}

pub trait System<'a> : 'static {
    type Inputs: SystemInputs<'a>;

    fn update(&mut self, inputs: Self::Inputs);
    fn on_event(&mut self, _event: &dyn AnyEvent) { }
}

pub trait SystemInputs<'a> {
    fn write_dependencies(output: &mut Vec<Dependency>);
    fn assemble(systems: &'a SystemMap, events: &'a EventQueueMap, calls: &'a CallQueueMap) -> Self;
}

#[derive(Clone, Debug)]
pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
    Call(SystemId),
    Emit(EventId),
}

impl<'a, S: System<'a>> SystemInputs<'a> for &'a S {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Read(SystemId::of::<S>()));
    }

    fn assemble(systems: &'a SystemMap, _events: &'a EventQueueMap, _calls: &'a CallQueueMap) -> Self {
        systems.get::<S>()
    }
}

impl<'a> SystemInputs<'a> for () {
    fn write_dependencies(_output: &mut Vec<Dependency>) { }
    fn assemble(_systems: &'a SystemMap, _events: &'a EventQueueMap, _calls: &'a CallQueueMap) -> Self { () }
}

impl<'a, A: SystemInputs<'a>, B: SystemInputs<'a>> SystemInputs<'a> for (A, B) {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        A::write_dependencies(output);
        B::write_dependencies(output);
    }

    fn assemble(systems: &'a SystemMap, events: &'a EventQueueMap, calls: &'a CallQueueMap) -> Self {
        (A::assemble(systems, events, calls), B::assemble(systems, events, calls))
    }
}

#[derive(Clone, Copy)]
pub struct Delay<'a, S>(&'a S);

impl<'a, S: System<'a>> SystemInputs<'a> for Delay<'a, S> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::ReadDelay(SystemId::of::<S>()));
    }

    fn assemble(systems: &'a SystemMap, _events: &'a EventQueueMap, _calls: &'a CallQueueMap) -> Self {
        Delay(systems.get::<S>())
    }
}

impl<'a, S> Deref for Delay<'a, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target { self.0 }
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<DynAnySystem>>>,
}

pub trait AnySystem<'a> {
    fn dependencies(&self) -> Vec<Dependency>;
    fn any_update(&mut self, systems: &'a SystemMap, events: &'a EventQueueMap, calls: &'a CallQueueMap);
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

    fn any_update(&mut self, systems: &'a SystemMap, events: &'a EventQueueMap, calls: &'a CallQueueMap) {
        self.update(S::Inputs::assemble(systems, events, calls));
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

    pub fn take_any(&mut self, id: SystemId) -> Box<DynAnySystem> { 
        self.systems.get_mut(&id)
            .expect("Can't take system that was not inserted")
            .take()
            .expect("Can't take system that was already taken")
    }

    pub fn untake_any(&mut self, sys: Box<DynAnySystem>) { 
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
                    Dependency::ReadDelay(dep_id) |
                    Dependency::Call(dep_id) => {
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

    pub fn iter_systems_mut(&mut self) -> impl Iterator<Item=&mut DynAnySystem> {
        self.systems.values_mut().map(|slot| slot.as_mut().unwrap().as_mut())
    }
}
