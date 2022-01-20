use std::{any::{Any, TypeId}, collections::{HashMap, HashSet}, iter, cmp::Ordering};

pub trait System: 'static {
    fn dependencies(&self) -> &'static [SystemDependency];

    fn update(&mut self, systems: &SystemContainer);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct SystemDependency {
    pub id: TypeId,
    pub type_: SystemDependencyType,
}

pub enum SystemDependencyType {
    Before,
    After,
}

#[derive(Default)]
pub struct SystemContainer(HashMap<TypeId, Box<dyn System>>);

impl SystemContainer {
    pub fn new() -> Self { 
        Default::default()
    }
    pub fn len(&self) -> usize { 
        self.0.len() 
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert<T: System>(&mut self, sys: T) -> Option<()> { 
        self.insert_with_box(Box::new(sys)).map(|_| ())
    }
    pub fn insert_with_box(&mut self, sys: Box<dyn System>) -> Option<Box<dyn System>> { 
        self.0.insert(sys.type_id(), sys)
    }

    pub fn remove<T: System>(&mut self) -> Option<()> { 
        self.remove_with_id(TypeId::of::<T>()).map(|_| ())
    }
    pub fn remove_with_id(&mut self, id: TypeId) -> Option<Box<dyn System>> { 
        self.0.remove(&id)
    }
    pub fn remove_with_ids(&mut self, ids: &[TypeId]) -> Self {
        ids.iter().filter_map(|&id| self.remove_with_id(id)).collect()
    }

    pub fn get<T: System>(&self) -> Option<&T> { 
        self.get_with_id(TypeId::of::<T>()).map(|s| s.as_any().downcast_ref().unwrap())
    }
    pub fn get_with_id(&self, id: TypeId) -> Option<&dyn System> { 
        self.0.get(&id).map(|s| &**s)
    }
    pub fn get_mut<T: System>(&mut self) -> Option<&mut T> { 
        self.get_mut_with_id(TypeId::of::<T>()).map(|s| s.as_any_mut().downcast_mut().unwrap())
    }
    pub fn get_mut_with_id(&mut self, id: TypeId) -> Option<&mut dyn System> { 
        self.0.get_mut(&id).map(|s| &mut **s)
    }

    pub fn contains<T: System>(&self) -> bool { 
        self.contains_id(TypeId::of::<T>()) 
    }
    pub fn contains_id(&self, id: TypeId) -> bool { 
        self.0.contains_key(&id) 
    }

    pub fn ids<'a>(&'a self) -> impl Iterator<Item = TypeId> + 'a { 
        self.0.keys().cloned()
    }
    pub fn systems<'a>(&'a self) -> impl Iterator<Item = &Box<dyn System>> + 'a { 
        self.0.values() 
    }
    pub fn into_systems(self) -> impl Iterator<Item = Box<dyn System>> { 
        self.0.into_values() 
    }
}

impl FromIterator<Box<dyn System>> for SystemContainer {
    fn from_iter<T: IntoIterator<Item = Box<dyn System>>>(iter: T) -> Self {
        SystemContainer(iter
            .into_iter()
            .map(|s| (s.type_id(), s))
            .collect())
    }
}

impl Extend<Box<dyn System>> for SystemContainer {
    fn extend<T: IntoIterator<Item = Box<dyn System>>>(&mut self, iter: T) {
        self.0.extend(iter
            .into_iter()
            .map(|sys| (sys.type_id(), sys)))
    }
}

pub type DependencyMap = HashMap<TypeId, HashSet<TypeId>>;

pub fn topological_sort(systems: &SystemContainer) -> Vec<TypeId> {
    let deps = build_dependency_map(systems);
    let mut unvisited = HashSet::from_iter(systems.ids());
    let mut result = Vec::new();

    fn visit(unvisited: &mut HashSet<TypeId>, result: &mut Vec<TypeId>, deps: &DependencyMap, id: TypeId)
    {
        if !unvisited.remove(&id) {
            return;
        }

        for &dep in deps.get(&id).iter().flat_map(|v| v.iter())
        {
            visit(unvisited, result, deps, dep);
        }

        result.push(id);
    }

    while let Some(&id) = unvisited.iter().next() {
        visit(&mut unvisited, &mut result, &deps, id);
    }

    result
}

fn build_dependency_map(systems: &SystemContainer) -> DependencyMap {
    let mut result = HashMap::new();
    for id in systems.ids() {
        let sys = systems.get_with_id(id).unwrap();
        for dep in sys.dependencies() {
            let (before, after) = match dep.type_ {
                SystemDependencyType::Before => (id, dep.id),
                SystemDependencyType::After => (dep.id, id),
            };
            result.entry(before).or_insert(HashSet::new()).insert(after);
        }
    }

    result
}

pub fn run_updates(all_systems: &mut SystemContainer, schedule: &[TypeId]) {
    for &id in schedule {
        let mut system = all_systems.remove_with_id(id).unwrap();
        let system_deps = SystemContainer::from_iter(
            system.dependencies().iter().map(|d| all_systems.remove_with_id(d.id).unwrap()));
        system.update(&system_deps);
        all_systems.extend(system_deps.into_systems());
    }
}