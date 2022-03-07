use std::collections::{HashMap, HashSet};
use std::slice;

use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::handler::{Context, Dependency, Handler};
use super::state::StateContainer;
use super::topic::TopicContainer;

pub struct Reactor {
    states: StateContainer,
    handlers_map: HashMap<EventId, Vec<Handler>>,
}

impl Reactor {
    pub fn new<'a>(states: StateContainer, handlers: Vec<Handler>) -> Self {
        let mut handlers_map: HashMap<EventId, Vec<Handler>> = HashMap::new();
        for handler in handlers {
            handlers_map
                .entry(handler.event_id())
                .or_default()
                .push(handler);
        }

        for handlers in handlers_map.values_mut() {
            sort_handlers_by_execution_order(handlers);
        }

        Reactor {
            states,
            handlers_map,
        }
    }

    pub fn states(&self) -> &StateContainer {
        &self.states
    }

    pub fn dispatch<E: Event>(&self, event: E) -> anyhow::Result<()> {
        let queue = EventQueue::new();
        queue.push(AnyEvent::new(event));
        while let Some(event) = queue.pop() {
            let topics = TopicContainer::new();
            if let Some(handlers) = self.handlers_map.get(&E::id()) {
                let context = Context {
                    states: &self.states,
                    queue: &queue,
                    topics: &topics,
                    event: &event,
                };

                for h in handlers {
                    h.call(&context)?;
                }
            }
        }

        Ok(())
    }
}

fn sort_handlers_by_execution_order(handlers: &mut Vec<Handler>) {
    let all_deps = handlers
        .iter()
        .map(|h| h.dependencies())
        .collect::<Vec<_>>();
    let order = execution_order(&all_deps);
    let mut handlers_temp = handlers.drain(..).map(Some).collect::<Vec<_>>();
    handlers.extend(
        order
            .iter()
            .map(|&idx| handlers_temp[idx].take().unwrap())
            .collect::<Vec<_>>(),
    );
}

fn execution_order(all_deps: &[&[Dependency]]) -> Vec<usize> {
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

                _ => {}
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
            for &child_idx in self.children.get(&idx).into_iter().flatten() {
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
