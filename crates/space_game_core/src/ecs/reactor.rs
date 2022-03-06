use std::{any::TypeId, collections::{HashMap, HashSet}, slice};

use super::{event::EventQueue, state::StateContainer, topic::TopicContainer, handler::{Handler, HandlerFn}, Event, AnyEvent};

pub struct Reactor {
    states: StateContainer,
    handlers_map: HashMap<TypeId, Vec<Handler>>,
}

impl Reactor {
    pub fn new<'a>(states: StateContainer, all_handlers: impl IntoIterator<Item=(TypeId, Vec<Handler>)>) -> Self {
        let mut handlers_map = HashMap::new();
        
        for (tid, handlers) in all_handlers {
            let all_deps = handlers
                .iter()
                .map(|h| h.dependencies)
                .collect::<Vec<_>>();
            let order = Dependency::execution_order(&all_deps);
            let mut handlers = handlers.into_iter().map(Some).collect::<Vec<_>>();
            let handlers_ordered = order.iter().map(|&idx| handlers[idx].take().unwrap()).collect::<Vec<_>>();
            assert!(handlers_map.insert(tid, handlers_ordered).is_none());
        }
        
        Reactor { states, handlers_map }
    }

    pub fn states(&self) -> &StateContainer {
        &self.states
    }

    pub fn dispatch<E: Event>(&self, event: E) -> anyhow::Result<()> {
        let events = EventQueue::new();
        events.push(AnyEvent::new(event));
        while let Some(event) = events.pop() {
            let topics = TopicContainer::new();
            if let Some(handlers) = self.handlers_map.get(&TypeId::of::<E>()) {
                for h in handlers {
                    (h.fn_box)(&event, &self.states, &events, &topics)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum Dependency {
    ReadState(TypeId),
    ReadStateDelayed(TypeId),
    WriteState(TypeId),
    SubscribeTopic(TypeId),
    PublishTopic(TypeId),
}

impl Dependency {
    pub fn execution_order(all_deps: &[&[Dependency]]) -> Vec<usize> {
        let mut writer = HashMap::new();
        let mut subscribers = HashMap::new();

        for (idx, &deps) in all_deps.iter().enumerate() {
            for dep in deps {
                match dep {
                    Dependency::WriteState(write_id) => {
                        if let Some(_conflict) = writer.insert(*write_id, idx) {
                            todo!();
                        }
                    }

                    Dependency::SubscribeTopic(topic_id) => {
                        subscribers.entry(topic_id).or_insert(Vec::new()).push(idx);
                    }

                    _ => {},
                }
            }
        }

        let mut children = HashMap::new();
        for (idx, &deps) in all_deps.iter().enumerate() {
            for dep in deps {
                let (parents, child) = match dep {
                    Dependency::ReadState(tid) => {
                        (slice::from_ref(&idx), *writer.get(&tid).unwrap())
                    }
                    Dependency::ReadStateDelayed(tid) => {
                        (slice::from_ref(writer.get(&tid).unwrap()), idx)
                    }
                    Dependency::PublishTopic(tid) => {
                        (subscribers.get(&tid).unwrap().as_slice(), idx)
                    }
                    Dependency::WriteState(_) | Dependency::SubscribeTopic(_) => continue,
                };

                for &parent in parents {
                    children.entry(parent).or_insert(Vec::new()).push(child);
                }
            }
        }

        struct Env<'s> {
            children: &'s HashMap<usize, Vec<usize>>,
            unvisited: HashSet<usize>,
            pending: HashSet<usize>,
            result: Vec<usize>,
        }

        impl Env<'_> {
            fn visit(&mut self, idx: usize) {
                if !self.unvisited.remove(&idx) {
                    return;
                }

                self.pending.insert(idx);
                for &child_idx in self.children.get(&idx).unwrap() {
                    if self.pending.contains(&child_idx) {
                        todo!();
                    }
                    self.visit(child_idx);
                }
                self.pending.remove(&idx);

                self.result.push(idx);
            }
        }

        let mut state = Env {
            children: &children,
            unvisited: (0..all_deps.len()).into_iter().collect(),
            pending: HashSet::new(),
            result: Vec::new(),
        };

        while let Some(&idx) = state.unvisited.iter().next() {
            state.visit(idx);
        }
        state.result
    }
}