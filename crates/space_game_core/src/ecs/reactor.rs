use std::collections::{HashMap, HashSet};
use std::slice;

use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::handler::{Context, Dependency, Handler, HandlerFn};
use super::state::StateContainer;
use super::topic::TopicContainer;

pub struct InitState;
impl Event for InitState {}

pub struct Reactor(HashMap<EventId, Vec<Handler>>);

impl Reactor {
    pub fn builder() -> ReactorBuilder {
        ReactorBuilder::default()
    }

    pub fn new<'a>(handlers: impl IntoIterator<Item = Handler>) -> Self {
        let mut result: HashMap<EventId, Vec<Handler>> = HashMap::new();
        for handler in handlers {
            result.entry(handler.event_id()).or_default().push(handler);
        }

        for handlers in result.values_mut() {
            sort_handlers_by_execution_order(handlers);
        }

        Reactor(result)
    }

    pub fn new_state(&self) -> anyhow::Result<StateContainer> {
        let states = StateContainer::new(
            self.0
                .values()
                .flatten()
                .flat_map(|h| h.dependencies().iter())
                .filter_map(|d| match d {
                    &Dependency::ReadState(id)
                    | &Dependency::ReadStateDelayed(id)
                    | &Dependency::WriteState(id) => Some(id),
                    _ => None,
                }),
        );
        self.dispatch(&states, InitState)?;
        Ok(states)
    }

    pub fn dispatch<E: Event>(&self, states: &StateContainer, event: E) -> anyhow::Result<()> {
        let queue = EventQueue::new();
        queue.push(AnyEvent::new(event));
        while let Some(event) = queue.pop() {
            if let Some(handlers) = self.0.get(&E::id()) {
                let topics = TopicContainer::new();
                let context = Context {
                    states,
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

#[derive(Default)]
pub struct ReactorBuilder(Vec<Handler>);

impl ReactorBuilder {
    pub fn add<E: Event, Args>(mut self, f: impl HandlerFn<E, Args>) -> Self {
        self.0.push(f.into_handler());
        self
    }

    pub fn build(self) -> Reactor {
        Reactor::new(self.0)
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
            .map(|&idx| handlers_temp[idx].take().expect("Execution order contains duplicate"))
            .collect::<Vec<_>>(),
    );
}

fn execution_order(all_deps: &[&[Dependency]]) -> Vec<usize> {
    let mut writers = HashMap::new();
    let mut subscribers = HashMap::new();

    for (idx, &deps) in all_deps.iter().enumerate() {
        for dep in deps {
            match dep {
                Dependency::WriteState(write_id) => {
                    if let Some(_conflict) = writers.insert(*write_id, idx) {
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
                    if let Some(writer) = writers.get(&tid) {
                        (slice::from_ref(&idx), *writer)
                    } else {
                        continue
                    }
                }
                Dependency::ReadStateDelayed(tid) => {
                    if let Some(writer) = writers.get(&tid) {
                        (slice::from_ref(writer), idx)
                    } else {
                        continue
                    }
                }
                Dependency::PublishTopic(tid) => {
                    if let Some(subs) = subscribers.get(&tid) {
                        (subs.as_slice(), idx)
                    } else {
                        continue
                    }
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
