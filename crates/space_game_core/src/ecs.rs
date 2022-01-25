use std::any::Any;
use std::collections::{HashMap, hash_map, HashSet};
use std::ops::{Index, IndexMut};

use slotmap::{new_key_type, SlotMap, SecondaryMap};

new_key_type! {
    pub struct EntityId;
    pub struct PrototypeId;
    pub struct SystemId;
}

pub struct Prototype {
    systems: HashSet<SystemId>,
    config: PrototypeConfig,
}

pub struct PrototypeConfig(HashMap<String, String>); // TODO better?

#[allow(unused)]
pub trait System {
    // fn pre_execute(&mut self) { }     // TODO
    fn execute(&mut self, systems: &SystemRefs<'_>, commands: &mut SystemCommands) { }
    fn dependencies(&self) -> &[Dependency] { &[] } // TODO make a SystemConfig to prevent this from changing

    fn init(&mut self, commands: &mut SystemCommands) { }

    // arg -> init
    fn create_entity(&mut self, id: EntityId, config: &PrototypeConfig, arg: Option<Box<dyn Any>>, commands: &mut SystemCommands) { }
    fn remove_entity(&mut self, id: EntityId, commands: &mut SystemCommands) { }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// TODO support both & and &mut.
pub struct SystemRefs<'a>(SecondaryMap<SystemId, &'a mut (dyn System + 'static)>);

#[derive(Default)]
pub struct SystemCommands(Vec<SystemCommand>);

impl SystemCommands {
    pub fn new() -> Self { Default::default() }
}

pub enum SystemCommand {
    New(PrototypeId, SecondaryMap<SystemId, Box<dyn Any>>),
    Remove(EntityId),
}

pub enum Dependency {
    ReadPrevious(SystemId),
    Write(SystemId),
    Read(SystemId),
}

impl Dependency {
    pub fn system_id(&self) -> SystemId {
        match self {
            Dependency::ReadPrevious(id) |
            Dependency::Write(id) | 
            Dependency::Read(id) => *id,
        }
    }
}

pub struct World {
    entities: SlotMap<EntityId, PrototypeId>,
    prototypes: NamedSlotMap<PrototypeId, Prototype>,
    systems: NamedSlotMap<SystemId, Option<Box<dyn System>>>,
    execution_order: Option<Vec<SystemId>>,
}

impl World {
    pub fn register_prototype(&mut self, name: String, proto: Prototype) -> PrototypeId {
        self.prototypes.insert(name, proto)
    }

    pub fn lookup_prototype(&mut self, name: &str) -> PrototypeId {
        self.prototypes.lookup_key(name)
    }

    pub fn get_prototype(&self, id: PrototypeId) -> &Prototype {
        &self.prototypes[id]
    }

    pub fn register_system_dyn(&mut self, name: String, system: Box<dyn System>) -> SystemId {
        self.execution_order = None;
        self.systems.insert(name, Some(system))
    }

    pub fn register_system<S: System + 'static>(&mut self, name: String, s: S) -> SystemId {
        self.register_system_dyn(name, Box::new(s))
    }

    pub fn lookup_system(&mut self, name: &str) -> SystemId {
        self.systems.lookup_key(name)
    }

    pub fn get_system<S: System + 'static>(&self, id: SystemId) -> &S {
        self.get_system_dyn(id).as_any().downcast_ref().unwrap()
    }

    pub fn get_system_dyn(&self, id: SystemId) -> &dyn System {
        &**self.systems[id].as_ref().unwrap()
    }
    
    pub fn get_system_mut<S: System + 'static>(&mut self, id: SystemId) -> &mut S {
        self.get_system_dyn_mut(id).as_any_mut().downcast_mut().unwrap()
    }

    pub fn get_system_dyn_mut(&mut self, id: SystemId) -> &mut dyn System {
        &mut **self.systems[id].as_mut().unwrap()
    }

    pub fn execute(&mut self) {
        let execution_order = self.execution_order.take().unwrap_or_else(|| self.compute_execution_order());
        let mut commands = SystemCommands::new();
        self.execute_systems(&mut commands, &execution_order);
        self.execute_commands(commands);
        self.execution_order = Some(execution_order);
    }

    fn compute_execution_order(&self) -> Vec<SystemId> { todo!() }

    fn execute_systems(&mut self, commands: &mut SystemCommands, order: &[SystemId]) {
        for &sys_id in order {
            let mut sys = self.systems[sys_id].take().unwrap();
            let mut deps = sys
                .dependencies()
                .iter()
                .map(|d| d.system_id())
                .map(|id| (id, self.systems[id].take().unwrap()))
                .collect::<Vec<_>>();
            let deps_refs = deps
                .iter_mut()
                .map(|(dep_id, dep)| (*dep_id, &mut **dep))
                .collect::<SecondaryMap<_, _>>();

            sys.execute(&SystemRefs(deps_refs), commands);
            
            for (dep_id, dep) in deps {
                self.systems[dep_id] = Some(dep);
            }
            self.systems[sys_id] = Some(sys);
        }
    }

    fn execute_commands(&mut self, mut commands: SystemCommands) -> bool {
        while !commands.0.is_empty() {
            let mut new_commands = SystemCommands::new();
            for cmd in commands.0 {
                match cmd {
                    SystemCommand::New(proto_id, mut args) => {
                        let id = self.entities.insert(proto_id);
                        let proto = &self.prototypes[proto_id];
                        for &sys_id in &proto.systems {
                            self.systems[sys_id].as_mut().unwrap().create_entity(id, &proto.config, args.remove(sys_id), &mut new_commands);
                        }
                    }
                    SystemCommand::Remove(id) => {
                        let proto_id = self.entities.remove(id).unwrap();
                        let proto = &self.prototypes[proto_id];
                        for &sys_id in &proto.systems {
                            self.systems[sys_id].as_mut().unwrap().remove_entity(id, &mut new_commands);
                        }
                    }
                }
            }
            commands = new_commands;
        }

        return false;
    }
}

#[derive(Default)]
pub struct NamedSlotMap<K: slotmap::Key, V> {
    slots: SlotMap<K, Option<V>>,
    names: SecondaryMap<K, String>,
    slots_by_name: HashMap<String, K>,
}

impl<K: slotmap::Key, V> NamedSlotMap<K, V> {
    pub fn new() -> Self {
        NamedSlotMap {
            slots: SlotMap::with_key(),
            names: SecondaryMap::new(),
            slots_by_name: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, val: V) -> K {
        match self.slots_by_name.entry(name) {
            hash_map::Entry::Occupied(entry) => {
                let key = *entry.get();
                let slot = &mut self.slots[key];
                if slot.is_some() {
                    panic!("Duplicate name `{}`", entry.key());
                }
                *slot = Some(val);
                key
            }
            hash_map::Entry::Vacant(entry) => {
                let key = self.slots.insert(Some(val));
                self.names.insert(key, entry.key().clone()).map(|_| panic!());
                entry.insert(key);
                key
            }
        }
    }

    pub fn lookup_key(&mut self, name: &str) -> K {
        if let Some(key) = self.slots_by_name.get(name) {
            return *key;
        }

        let key = self.slots.insert(None);
        self.names.insert(key, name.to_string()).map(|_| panic!());
        self.slots_by_name.insert(name.to_string(), key).map(|_| panic!());
        key
    }

    pub fn lookup_name(&self, key: K) -> &str {
        &self.names[key]
    }

    pub fn remove(&mut self, key: K) -> V {
        let val = self.slots.remove(key).unwrap().unwrap();
        let name = self.names.remove(key).unwrap();
        self.slots_by_name.remove(&name).unwrap();
        val
    }

    pub fn iter_missing_lookups(&self) -> impl Iterator<Item=K> + '_ {
        self.slots.iter().filter_map(|(k, v)| {
            if v.is_none() {
                Some(k)
            } else {
                None
            }
        })
    }
}

impl<K: slotmap::Key, V> Index<K> for NamedSlotMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.slots[index].as_ref().unwrap()
    }
}

impl<K: slotmap::Key, V> IndexMut<K> for NamedSlotMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.slots[index].as_mut().unwrap()
    }
}

