use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};

use slotmap::{new_key_type, SlotMap, SecondaryMap};

new_key_type! {
    pub struct EntityId;
    pub struct EntityPrototypeId;
    pub struct SystemId;
}

pub struct EntityPrototype {
    name: String,
    systems: Vec<SystemId>,
}

pub trait System {
    fn execute(&mut self, deps: &SystemRefs<'_>, commands: &mut EntityCommands);
    
    fn add_entity(&mut self, id: EntityId, arg: Option<Box<dyn Any>>);
    fn remove_entity(&mut self, id: EntityId);

    fn dependencies(&self) -> &[Dependency];

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct SystemRefs<'a>(SecondaryMap<SystemId, &'a dyn System>);
pub struct SystemRefsMut<'a>(SecondaryMap<SystemId, &'a mut dyn System>);

pub struct EntityCommands {
    add_entities: Vec<(EntityPrototypeId, SecondaryMap<SystemId, Box<dyn Any>>)>,
    remove_entities: HashSet<EntityId>,
}

pub struct Dependency {
    system: SystemId,
    delayed: bool,
}

pub struct World {
    entities: SlotMap<EntityId, EntityPrototypeId>,
    entity_prototypes: SlotMap<EntityPrototypeId, EntityPrototype>,
    systems: SlotMap<SystemId, Box<dyn System>>,
    systems_by_type: HashMap<TypeId, SystemId>,
}