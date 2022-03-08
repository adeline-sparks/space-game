use std::collections::{hash_map, HashMap, HashSet};
use std::slice::{self};

use thiserror::Error;

use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::handler::{Context, Dependency, Handler, HandlerFn};
use super::state::{StateContainer, StateId};
use super::topic::TopicContainer;

pub struct InitState;
impl Event for InitState {}

pub struct Reactor(HashMap<EventId, Vec<Handler>>);

impl Reactor {
    pub fn builder() -> ReactorBuilder {
        ReactorBuilder::default()
    }

    pub fn new<'a>(
        handlers: impl IntoIterator<Item = Handler>,
    ) -> Result<Self, NoExecutionOrderError> {
        let mut result: HashMap<EventId, Vec<Handler>> = HashMap::new();
        for handler in handlers {
            result.entry(handler.event_id()).or_default().push(handler);
        }

        for handlers in result.values_mut() {
            sort_handlers_by_execution_order(handlers)?;
        }

        Ok(Reactor(result))
    }

    pub fn new_state(&self) -> anyhow::Result<StateContainer> {
        let states = StateContainer::new(
            self.0
                .values()
                .flatten()
                .flat_map(|h| h.dependencies().iter())
                .filter_map(|d| match d {
                    Dependency::ReadState(id)
                    | Dependency::ReadStateDelayed(id)
                    | Dependency::WriteState(id) => Some(id),
                    _ => None,
                })
                .cloned(),
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

    pub fn build(self) -> Result<Reactor, NoExecutionOrderError> {
        Reactor::new(self.0)
    }
}

#[derive(Error, Debug)]
#[error("Handlers have no possible execution order: {0}")]
pub struct NoExecutionOrderError(String);

fn sort_handlers_by_execution_order(
    handlers: &mut Vec<Handler>,
) -> Result<(), NoExecutionOrderError> {
    let all_deps = handlers
        .iter()
        .map(|h| h.dependencies())
        .collect::<Vec<_>>();
    let order = match execution_order(&all_deps) {
        Ok(order) => order,
        Err(errors) => {
            let message = errors
                .iter()
                .map(|e| e.error_message(|idx| handlers[idx].to_string()))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(NoExecutionOrderError(message));
        }
    };

    let mut handlers_temp = handlers.drain(..).map(Some).collect::<Vec<_>>();
    handlers.extend(
        order
            .iter()
            .map(|&idx| {
                handlers_temp[idx]
                    .take()
                    .expect("Execution order contains duplicate")
            })
            .collect::<Vec<_>>(),
    );

    Ok(())
}

enum ExecutionOrderError {
    WriteConflict(StateId, usize, usize),
    Cyclic(Vec<usize>),
}

impl ExecutionOrderError {
    fn error_message<'a>(&self, get_name: impl Fn(usize) -> String) -> String {
        match self {
            &ExecutionOrderError::WriteConflict(ref state_id, a, b) => {
                let a_name = get_name(a);
                let b_name = get_name(b);
                format!("Write conflict on {state_id}. Writers are {a_name} and {b_name}.")
            }
            ExecutionOrderError::Cyclic(ids) => {
                let names = ids.iter().cloned().map(get_name).collect::<Vec<_>>();
                format!("Cyclic dependency between {}.", names.join(", "))
            }
        }
    }
}

fn execution_order(all_deps: &[&[Dependency]]) -> Result<Vec<usize>, Vec<ExecutionOrderError>> {
    let mut errors = Vec::new();

    let mut writers = HashMap::new();
    let mut subscribers = HashMap::new();

    for (idx, &deps) in all_deps.iter().enumerate() {
        for dep in deps {
            match dep {
                Dependency::WriteState(write_id) => match writers.entry(write_id.clone()) {
                    hash_map::Entry::Vacant(entry) => {
                        entry.insert(idx);
                    }
                    hash_map::Entry::Occupied(entry) => {
                        errors.push(ExecutionOrderError::WriteConflict(
                            write_id.clone(),
                            idx,
                            *entry.get(),
                        ));
                    }
                },

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
                        continue;
                    }
                }
                Dependency::ReadStateDelayed(tid) => {
                    if let Some(writer) = writers.get(&tid) {
                        (slice::from_ref(writer), idx)
                    } else {
                        continue;
                    }
                }
                Dependency::PublishTopic(tid) => {
                    if let Some(subs) = subscribers.get(&tid) {
                        (subs.as_slice(), idx)
                    } else {
                        continue;
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
        pending_stack: Vec<usize>,
        result: Vec<usize>,
        errors: &'s mut Vec<ExecutionOrderError>,
    }

    impl Env<'_> {
        fn visit(&mut self, idx: usize) {
            if !self.unvisited.remove(&idx) {
                return;
            }

            self.pending.insert(idx);
            self.pending_stack.push(idx);
            for &child_idx in self.children.get(&idx).into_iter().flatten() {
                if !self.pending.contains(&child_idx) {
                    self.visit(child_idx);
                } else {
                    let mut cycle = self.pending_stack.clone();
                    cycle.reverse();
                    self.errors.push(ExecutionOrderError::Cyclic(cycle));
                }
            }
            self.pending.remove(&idx);
            self.pending_stack.pop();

            self.result.push(idx);
        }
    }

    let mut state = Env {
        children: &children,
        unvisited: (0..all_deps.len()).into_iter().collect(),
        pending: HashSet::new(),
        pending_stack: Vec::new(),
        result: Vec::new(),
        errors: &mut errors,
    };

    while let Some(&idx) = state.unvisited.iter().next() {
        state.visit(idx);
    }
    Ok(state.result)
}
