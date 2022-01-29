use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use impl_trait_for_tuples::impl_for_tuples;

use super::{World, EntityId, ArchetypeId};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SystemId(TypeId);

impl SystemId {
    pub fn of<'a, S: System<'a>>() -> Self {
        Self(TypeId::of::<S>())
    }

    pub fn type_id(self) -> TypeId {
        self.0
    }
}

impl From<&DynAnySystem> for SystemId {
    fn from(sys: &DynAnySystem) -> Self {
        SystemId(sys.as_any().type_id())
    }
}

pub trait System<'a>: 'static {
    type Inputs: SystemInputs<'a>;

    fn update(&mut self, _inputs: Self::Inputs);

    fn create_entity(&mut self, _id: EntityId, _arch_id: ArchetypeId) { }
    fn destroy_entity(&mut self, _id: EntityId) { }
}

pub trait SystemInputs<'a> {
    fn write_dependencies(output: &mut Vec<Dependency>);
    fn assemble(world: &'a World) -> Self;
}

#[derive(Clone, Debug)]
pub enum Dependency {
    Read(SystemId),
    ReadDelay(SystemId),
    Call(SystemId),
}

impl<'a, S: System<'a>> SystemInputs<'a> for &'a S {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::Read(SystemId::of::<S>()));
    }

    fn assemble(world: &'a World) -> Self {
        world.get::<S>()
    }
}

#[impl_for_tuples(5)]
impl<'a> SystemInputs<'a> for Tuple {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        for_tuples!(#(Tuple::write_dependencies(output);)*);
    }

    fn assemble(world: &'a World) -> Self {
        (for_tuples!(#(Tuple::assemble(world)),*))
    }
}

#[derive(Clone, Copy)]
pub struct Delay<'a, S>(&'a S);

impl<'a, S: System<'a>> SystemInputs<'a> for Delay<'a, S> {
    fn write_dependencies(output: &mut Vec<Dependency>) {
        output.push(Dependency::ReadDelay(SystemId::of::<S>()));
    }

    fn assemble(world: &'a World) -> Self {
        Delay(world.systems.get::<S>())
    }
}

impl<'a, S> Deref for Delay<'a, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Default)]
pub struct SystemMap {
    systems: HashMap<SystemId, Option<Box<DynAnySystem>>>,
}

pub trait AnySystem<'a> {
    fn dependencies(&self) -> Vec<Dependency>;
    fn update(&mut self, world: &'a World);
    fn create_entity(&mut self, id: EntityId, arch_id: ArchetypeId);
    fn destory_entity(&mut self, id: EntityId);

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

    fn update(&mut self, world: &'a World) {
        S::update(self, S::Inputs::assemble(world));
    }

    fn create_entity(&mut self, id: EntityId, arch_id: ArchetypeId) {
        S::create_entity(self, id, arch_id)
    }

    fn destory_entity(&mut self, id: EntityId) {
        S::destroy_entity(self, id)
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

impl SystemMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert<S: for<'a> System<'a>>(&mut self, sys: S) {
        if self
            .systems
            .insert(SystemId::of::<S>(), Some(Box::new(sys)))
            .is_some()
        {
            panic!("Can't insert duplicate system");
        }
    }
    pub fn remove<'a, S: System<'a>>(&mut self) -> S {
        *self
            .systems
            .remove(&SystemId::of::<S>())
            .expect("Can't remove system that was not inserted")
            .expect("Can't remove system that was taken")
            .as_any_box()
            .downcast()
            .unwrap()
    }

    pub fn get<'a, S: System<'a>>(&self) -> &S {
        self.systems
            .get(&SystemId::of::<S>())
            .expect("Can't get system that was not inserted")
            .as_ref()
            .expect("Can't get system that was taken")
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    pub fn get_mut<'a, S: System<'a>>(&mut self) -> &mut S {
        self.systems
            .get_mut(&SystemId::of::<S>())
            .expect("Can't get system that was not inserted")
            .as_mut()
            .expect("Can't get system that was taken")
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    pub fn take_any(&mut self, id: SystemId) -> Box<DynAnySystem> {
        self.systems
            .get_mut(&id)
            .expect("Can't take system that was not inserted")
            .take()
            .expect("Can't take system that was already taken")
    }

    pub fn untake_any(&mut self, sys: Box<DynAnySystem>) {
        let id = SystemId::from(sys.as_ref());
        let sys_opt = self
            .systems
            .get_mut(&id)
            .expect("Can't untake system that was never inserted");
        if sys_opt.is_some() {
            panic!("Can't untake system that was never taken");
        }
        *sys_opt = Some(sys);
    }

    pub fn topological_order(&self) -> Result<Vec<SystemId>, ()> {
        let mut dep_map = HashMap::<SystemId, Vec<SystemId>>::new();
        for sys in self.systems.values() {
            let sys = sys
                .as_deref()
                .expect("Can't compute topological_order with taken System(s)");
            let sys_id = SystemId::from(sys);
            for dep in sys.dependencies() {
                match dep {
                    Dependency::Read(dep_id) => {
                        dep_map.entry(sys_id).or_default().push(dep_id);
                    }
                    Dependency::ReadDelay(dep_id) | Dependency::Call(dep_id) => {
                        dep_map.entry(dep_id).or_default().push(sys_id);
                    }
                }
            }
        }
        let dep_map = dep_map;

        fn visit(
            id: SystemId,
            dep_map: &HashMap<SystemId, Vec<SystemId>>,
            unvisited: &mut HashSet<SystemId>,
            pending: &mut HashSet<SystemId>,
            result: &mut Vec<SystemId>,
        ) -> Result<(), ()> {
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

    pub fn iter_systems_mut(&mut self) -> impl Iterator<Item = &mut DynAnySystem> {
        self.systems
            .values_mut()
            .map(|slot| slot.as_mut().unwrap().as_mut())
    }
}
