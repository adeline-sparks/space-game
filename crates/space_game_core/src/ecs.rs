use std::{any::{Any, TypeId}, collections::{HashMap, HashSet}};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<'a, S: System<'a>>() -> Self { SystemId(TypeId::of::<S>()) }
}

pub trait System<'a> : 'static {
    type Inputs: SystemInputs<'a>;

    fn update(&mut self, inputs: Self::Inputs);
}

pub trait SystemInputs<'a> {
    fn write_dependencies(output: &mut Vec<Dependency>);
    fn assemble(systems: &'a SystemMap) -> Self;
}

#[derive(Clone, Debug)]
pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
}

impl<'a, S: System<'a>> SystemInputs<'a> for &'a S {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Read(SystemId::of::<S>()));
    }

    fn assemble(systems: &'a SystemMap) -> Self {
        systems.get::<S>()
    }
}

impl<'a> SystemInputs<'a> for () {
    fn write_dependencies(_output: &mut Vec<Dependency>) { }
    fn assemble(_systems: &'a SystemMap) -> Self { () }
}

impl<'a, A: SystemInputs<'a>, B: SystemInputs<'a>> SystemInputs<'a> for (A, B) {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        A::write_dependencies(output);
        B::write_dependencies(output);
    }

    fn assemble(systems: &'a SystemMap) -> Self {
        (A::assemble(systems), B::assemble(systems))
    }
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<DynAnySystem>>>,
}

trait AnySystem<'a> {
    fn dependencies(&self) -> Vec<Dependency>;
    fn assemble_update(&mut self, systems: &'a SystemMap);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

impl<'a, S: System<'a>> AnySystem<'a> for S {
    fn dependencies(&self) -> Vec<Dependency> {
        let mut result = Vec::new();
        S::Inputs::write_dependencies(&mut result);
        result
    }

    fn assemble_update(&mut self, systems: &'a SystemMap) {
        self.update(S::Inputs::assemble(systems));
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

type DynAnySystem = dyn for<'a> AnySystem<'a>;

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
            .get_or_insert_with(|| self.systems.topological_order().unwrap())
            .as_slice();

        for &id in order {
            let mut sys = self.systems.take_any(id);
            sys.assemble_update(&self.systems);
            self.systems.untake_any(sys);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple() {
        struct SysA(usize);
        struct SysB(usize);
        struct SysC(usize);

        impl<'a> System<'a> for SysA {
            type Inputs = ();

            fn update(&mut self, _systems: ()) { self.0 += 1; }
        }

        impl<'a> System<'a> for SysB {
            type Inputs = ();

            fn update(&mut self, _systems: ()) { self.0 += 2; }
        }

        impl<'a> System<'a> for SysC {
            type Inputs = (&'a SysA, &'a SysB);

            fn update(&mut self, inputs: Self::Inputs) { 
                self.0 = inputs.0.0 + inputs.1.0;
            }
        }

        let mut world = World::new();
        world.systems_mut().insert(SysA(0));
        world.systems_mut().insert(SysB(0));
        world.systems_mut().insert(SysC(0));

        for i in 0..10 {
            world.update();
            assert_eq!(world.systems().get::<SysC>().0, 3 * (i + 1));
        }
    }
}
