use slotmap::{new_key_type, SlotMap};

use super::{State, Topic, HandlerGroup, Subscriber, Writer};

new_key_type! {
    pub struct EntityId;
    pub struct ArchetypeId;
}

#[derive(Debug, Clone)]
pub struct Archetype;

#[derive(Debug)]
pub struct CreateEntity(ArchetypeId);
impl Topic for CreateEntity { }

#[derive(Debug)]
pub struct DestroyEntity(EntityId);
impl Topic for DestroyEntity { }

#[derive(Default, Clone)]
pub struct EntityState {
    entity_map: SlotMap<EntityId, ArchetypeId>,
    #[allow(unused)]
    archetype_map: SlotMap<ArchetypeId, Archetype>,
}
impl State for EntityState { }

impl HandlerGroup for EntityState {
    fn add_group(builder: super::reactor::ReactorBuilder) -> super::reactor::ReactorBuilder {
        builder
            .add_global(|creates: Subscriber<CreateEntity>, destroys: Subscriber<DestroyEntity>, mut state: Writer<EntityState>| -> anyhow::Result<()> {
                for destroy in destroys.iter() {
                    state.entity_map.remove(destroy.0);
                }
                for create in creates.iter() {
                    state.entity_map.insert(create.0);
                }

                Ok(())
            })
    }
}
