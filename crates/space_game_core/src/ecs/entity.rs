use std::collections::HashMap;

use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct EntityId;
    pub struct ArchetypeId;
}

pub struct Archetype {
    name: String,
    config: ArchetypeConfig,
}

pub type ArchetypeConfig = toml::value::Table;

#[derive(Default)]
pub struct EntityMap {
    entities: SlotMap<EntityId, ArchetypeId>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
    archetypes_by_name: HashMap<String, ArchetypeId>,
}

impl EntityMap {
    pub fn new() -> Self { Default::default() }

    pub fn add_archetype(&mut self, arch: Archetype) {
        let name = arch.name.clone();
        let id = self.archetypes.insert(arch);
        if self.archetypes_by_name.insert(name.clone(), id).is_some() {
            panic!("Added duplicate archetype `{name}`")
        }
    }

    pub fn get_archetype_id(&self, name: &str) -> Option<ArchetypeId> {
        self.archetypes_by_name.get(name).cloned()
    }

    pub fn get_archetype(&self, arch_id: ArchetypeId) -> &Archetype {
        &self.archetypes[arch_id]
    }

    pub fn create_entity(&mut self, arch_id: ArchetypeId) -> EntityId {
        self.entities.insert(arch_id)
    }

    pub fn destroy_entity(&mut self, id: EntityId) {
        if self.entities.remove(id).is_none() {
            panic!("Destroyed invalid EntityId {id:?}");
        }
    }
}