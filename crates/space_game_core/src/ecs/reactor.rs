use std::collections::HashMap;

use thiserror::Error;

use super::dependency::{execution_order, Dependency};
use super::event::{AnyEvent, Event, EventId, EventQueue};
use super::handler::{Context, Handler, HandlerFn};
use super::state::StateContainer;
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
            result.entry(handler.event_id().clone()).or_default().push(handler);
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
