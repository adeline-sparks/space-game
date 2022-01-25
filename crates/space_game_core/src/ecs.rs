use std::{any::{Any, TypeId}, collections::HashMap, ops::Deref};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<S: System>() -> Self { SystemId(TypeId::of::<S>()) }
}

impl From<&dyn ErasedSystem> for SystemId {
    fn from(sys: &dyn ErasedSystem) -> Self {
        SystemId(sys.as_any().type_id())
    }
}

pub trait System : Sized + 'static {
    type Inputs : for <'a> SystemInputs<'a>;

    fn update(&mut self, inputs: Self::Inputs);
}

pub trait SystemInputs<'a> {
    fn from(systems: &'a SystemMap) -> Self;

    fn write_dependencies(out: &mut Vec<Dependency>);
}

pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
}

impl<'a, S: System> SystemInputs<'a> for &'a S {
    fn from(systems: &'a SystemMap) -> Self {
        systems.get::<S>()
    }

    fn write_dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::Read(SystemId::of::<S>()));
    }
}

struct Delay<S>(S);

impl<'a, S: System> SystemInputs<'a> for Delay<&'a S> {
    fn from(systems: &'a SystemMap) -> Self {
        Delay(systems.get::<S>())
    }

    fn write_dependencies(out: &mut Vec<Dependency>) {
        out.push(Dependency::ReadDelay(SystemId::of::<S>()));
    }
}

impl<'a, A: SystemInputs<'a>> SystemInputs<'a> for (A,) {
    fn from(systems: &'a SystemMap) -> Self {
        (A::from(systems),)
    }

    fn write_dependencies(out: &mut Vec<Dependency>) {
        A::write_dependencies(out);
    }
}

impl<'a, A: SystemInputs<'a>, B: SystemInputs<'a>> SystemInputs<'a> for (A, B) {
    fn from(systems: &'a SystemMap) -> Self {
        (A::from(systems), B::from(systems))
    }

    fn write_dependencies(out: &mut Vec<Dependency>) {
        A::write_dependencies(out);
        B::write_dependencies(out);
    }
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<dyn ErasedSystem>>>,
}

trait ErasedSystem {
    fn update_erased(&mut self, systems: &SystemMap);
    fn get_dependencies_erased(&self) -> Vec<Dependency>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

impl SystemMap {
    pub fn new() -> Self { Default::default() }

    pub fn insert<S: System>(&mut self, sys: S) { 
        if self.systems.insert(SystemId::of::<S>(), Some(Box::new(sys))).is_some() {
            panic!("Can't insert duplicate system");
        }
    }
    pub fn remove<S: System>(&mut self) -> S {
        *self.systems.remove(&SystemId::of::<S>())
            .expect("Can't remove system that was not inserted")
            .expect("Can't remove system that was taken")
            .as_any_box()
            .downcast()
            .unwrap()
    }

    pub fn get<S: System>(&self) -> &S { 
        self.systems.get(&SystemId::of::<S>())
            .expect("Can't get system that was not inserted")
            .as_ref()
            .expect("Can't get system that was taken")
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn get_erased(&self, id: SystemId) -> &dyn ErasedSystem {
        self.systems.get(&id)
            .expect("Can't get system that was not inserted")
            .as_ref()
            .expect("Can't get system that was taken")
            .deref()
    }

    fn take_erased(&mut self, id: SystemId) -> Box<dyn ErasedSystem> { 
        self.systems.get_mut(&id)
            .expect("Can't take system that was not inserted")
            .take()
            .expect("Can't take system that was already taken")
    }

    fn untake_erased(&mut self, sys: Box<dyn ErasedSystem>) { 
        let id = SystemId::from(sys.as_ref());
        let sys_opt = self.systems.get_mut(&id)
            .expect("Can't untake system that was never inserted");
        if sys_opt.is_some() {
            panic!("Can't untake system that was never taken");
        }
        *sys_opt = Some(sys);
    }

    pub fn topological_order(&self) -> Vec<SystemId> { 
        let mut dep_map = HashMap::<SystemId, Vec<SystemId>>::new();
        for sys in self.systems.values() {
            let sys = sys.as_ref().expect("Can't compute topological_order with taken System(s)");
        }


        let mut result = Vec::new();
        todo!()
    }
}

impl<I: for<'a> SystemInputs<'a>, S: System<Inputs=I>> ErasedSystem for S {
    fn update_erased(&mut self, systems: &SystemMap) {
        self.update(I::from(systems));
    }

    fn get_dependencies_erased(&self) -> Vec<Dependency> {
        let mut result = Vec::new();
        I::write_dependencies(&mut result);
        result
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> { self }
}

#[derive(Default)]
pub struct World {
    systems: SystemMap,
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

    pub fn update(&mut self) {
        let order = self.topological_order
            .get_or_insert_with(|| self.systems.topological_order())
            .as_slice();

        for &id in order {
            let mut sys = self.systems.take_erased(id);
            sys.update_erased(&self.systems);
            self.systems.untake_erased(sys);
        }
    }
}