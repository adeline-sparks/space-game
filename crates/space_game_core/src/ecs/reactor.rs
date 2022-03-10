use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use log::error;
use thiserror::Error;

use super::dependency::execution_order;
use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::handler::{Context, Handler, HandlerFn};
use super::state::StateContainer;
use super::topic::TopicContainer;

#[derive(Debug)]
pub struct InitEvent;
impl Event for InitEvent {}

pub struct Reactor(HashMap<EventId, Vec<Handler>>);

impl Reactor {
    pub fn builder() -> ReactorBuilder {
        ReactorBuilder::default()
    }

    pub fn new<'a>(
        handlers: impl IntoIterator<Item = Handler>,
    ) -> Result<Self, ExecutionOrderConflictError> {
        let mut result: HashMap<EventId, Vec<Handler>> = HashMap::new();
        for handler in handlers {
            result
                .entry(handler.event_id().clone())
                .or_default()
                .push(handler);
        }

        for handlers in result.values_mut() {
            sort_handlers_by_execution_order(handlers)?;
        }

        Ok(Reactor(result))
    }

    pub fn new_state_container(&self) -> StateContainer {
        let states = StateContainer::new(
            self.0
                .values()
                .flatten()
                .flat_map(|h| h.dependencies().iter())
                .filter_map(|d| d.state_id().cloned())
                .collect::<HashSet<_>>(),
        );

        self.dispatch(&states, InitEvent);
        states
    }

    pub fn dispatch<E: Event>(&self, states: &StateContainer, event: E) {
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
                    match h.call(&context) {
                        Ok(()) => {}
                        Err(err) => {
                            error!("Handler '{}' failed while handling {:?}: {}", h, event, err)
                        }
                    }
                }
            }
        }
    }
}

#[derive(Error, Debug)]
pub struct ExecutionOrderConflictError(Vec<String>);

impl Display for ExecutionOrderConflictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Found dependency conflict(s) in execution order.\n")?;
        for msg in &self.0 {
            write!(f, "{}\n", msg)?;
        }

        Ok(())
    }
}

fn sort_handlers_by_execution_order(
    handlers: &mut Vec<Handler>,
) -> Result<(), ExecutionOrderConflictError> {
    let all_deps = handlers
        .iter()
        .map(|h| h.dependencies())
        .collect::<Vec<_>>();
    let order = match execution_order(&all_deps) {
        Ok(order) => order,
        Err(errors) => {
            let messages = errors
                .iter()
                .map(|e| e.error_message(|idx| handlers[idx].to_string()))
                .collect::<Vec<_>>();
            return Err(ExecutionOrderConflictError(messages));
        }
    };

    let mut handlers_temp = handlers.drain(..).map(Some).collect::<Vec<_>>();
    handlers.extend(
        order
            .iter()
            .map(|&idx| {
                handlers_temp[idx]
                    .take()
                    .expect("execution_order() returned a duplicate")
            })
            .collect::<Vec<_>>(),
    );

    Ok(())
}

#[derive(Default)]
pub struct ReactorBuilder(Vec<Handler>);

impl ReactorBuilder {
    pub fn add<E: Event, Args>(mut self, f: impl HandlerFn<E, Args>) -> Self {
        self.0.push(f.into_handler());
        self
    }

    pub fn build(self) -> Result<Reactor, ExecutionOrderConflictError> {
        Reactor::new(self.0)
    }
}
