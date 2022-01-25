use std::{any::{Any, TypeId}, collections::{HashMap, HashSet}};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<S: System>() -> Self { SystemId(TypeId::of::<S>()) }
}

impl From<&dyn AnySystem> for SystemId {
    fn from(sys: &dyn AnySystem) -> Self {
        SystemId(sys.as_any().type_id())
    }
}

pub trait System : 'static {
    fn dependencies(&self) -> Vec<Dependency>;
    fn update(&mut self, systems: &SystemMap);
}

#[derive(Clone, Debug)]
pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<dyn AnySystem>>>,
}

trait AnySystem : System {
    fn as_any(&self) -> &dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: System + Sized> AnySystem for T {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_box(self: Box<Self>) -> Box<dyn Any> { self }
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

    fn take_any(&mut self, id: SystemId) -> Box<dyn AnySystem> { 
        self.systems.get_mut(&id)
            .expect("Can't take system that was not inserted")
            .take()
            .expect("Can't take system that was already taken")
    }

    fn untake_any(&mut self, sys: Box<dyn AnySystem>) { 
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
            sys.update(&self.systems);
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

        impl System for SysA {
            fn dependencies(&self) -> Vec<Dependency> { vec![] }
            fn update(&mut self, _systems: &SystemMap) { self.0 += 1; }
        }

        impl System for SysB {
            fn dependencies(&self) -> Vec<Dependency> { vec![] }
            fn update(&mut self, _systems: &SystemMap) { self.0 += 2; }
        }

        impl System for SysC {
            fn dependencies(&self) -> Vec<Dependency> { 
                vec![
                    Dependency::Read(SystemId::of::<SysA>()), 
                    Dependency::Read(SystemId::of::<SysB>()), 
                ]
            }
            fn update(&mut self, systems: &SystemMap) { 
                self.0 = systems.get::<SysA>().0 + systems.get::<SysB>().0;
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