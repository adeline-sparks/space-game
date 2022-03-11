use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use log::error;
use petgraph::algo::kosaraju_scc;
use petgraph::graph::DiGraph;
use thiserror::Error;

use crate::ecs::handler::Dependency;
use crate::ecs::state::StateId;
use crate::ecs::topic::TopicId;

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
        let topics = TopicContainer::new();

        let queue = EventQueue::new();
        queue.push(AnyEvent::new(event));
        while let Some(event) = queue.pop() {
            let handlers = match self.0.get(&E::id()) {
                Some(handlers) => handlers,
                None => continue,
            };

            topics.clear();
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

#[derive(Default)]
pub struct ReactorBuilder(Vec<Handler>);

#[derive(Error, Debug)]
pub enum BuildReactorError {
    #[error("While processing {0}: {1}")]
    Cycle(EventId, CyclicDependenciesError),
}

impl ReactorBuilder {
    pub fn add<E: Event, Args>(mut self, f: impl HandlerFn<E, Args>) -> Self {
        self.0.push(f.into_handler());
        self
    }

    pub fn build(self) -> Result<Reactor, BuildReactorError> {
        let mut result: HashMap<EventId, Vec<Handler>> = HashMap::new();
        for handler in self.0 {
            result
                .entry(handler.event_id().clone())
                .or_default()
                .push(handler);
        }

        for (event_id, handlers) in result.iter_mut() {
            sort_handlers_by_execution_order(handlers)
                .map_err(|err| BuildReactorError::Cycle(event_id.clone(), err))?;
        }

        Ok(Reactor(result))
    }
}

#[derive(Error, Debug)]
pub struct CyclicDependenciesError(Vec<String>);

impl Display for CyclicDependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cyclic dependency between: ")?;
        for (i, name) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(name)?;
        }

        Ok(())
    }
}

fn sort_handlers_by_execution_order(
    handlers: &mut Vec<Handler>,
) -> Result<(), CyclicDependenciesError> {
    enum Node {
        Handler(usize),
        State(StateId),
        Topic(TopicId),
    }

    let mut graph = DiGraph::<Node, ()>::new();
    let mut handler_nodes = Vec::new();
    let mut state_nodes = HashMap::new();
    let mut topic_nodes = HashMap::new();

    for (idx, handler) in handlers.iter().enumerate() {
        for dep in handler.dependencies() {
            match dep {
                Dependency::ReadState(id)
                | Dependency::ReadStateDelayed(id)
                | Dependency::WriteState(id) => {
                    state_nodes
                        .entry(id.clone())
                        .or_insert_with(|| graph.add_node(Node::State(id.clone())));
                }
                Dependency::PublishTopic(id) | Dependency::SubscribeTopic(id) => {
                    topic_nodes
                        .entry(id.clone())
                        .or_insert_with(|| graph.add_node(Node::Topic(id.clone())));
                }
            }
        }

        handler_nodes.push(graph.add_node(Node::Handler(idx)));
    }

    for &handler_node in &handler_nodes {
        let handler = match &graph[handler_node] {
            &Node::Handler(idx) => &handlers[idx],
            _ => panic!("Non-handler found at handler_node index"),
        };

        for dep in handler.dependencies() {
            match dep {
                Dependency::ReadState(id) => {
                    graph.add_edge(state_nodes[id], handler_node, ());
                }
                Dependency::ReadStateDelayed(id) | Dependency::WriteState(id) => {
                    graph.add_edge(handler_node, state_nodes[id], ());
                }
                Dependency::SubscribeTopic(id) => {
                    graph.add_edge(topic_nodes[id], handler_node, ());
                }
                Dependency::PublishTopic(id) => {
                    graph.add_edge(handler_node, topic_nodes[id], ());
                }
            }
        }
    }

    let mut sccs = kosaraju_scc(&graph);
    sccs.reverse();

    let mut handlers_temp = handlers.drain(..).map(Some).collect::<Vec<_>>();
    for scc in sccs {
        if scc.len() > 1 {
            let names = scc
                .iter()
                .map(|&node| match &graph[node] {
                    &Node::Handler(idx) => format!("Handler {}", handlers[idx]),
                    Node::State(id) => format!("State {}", id),
                    Node::Topic(id) => format!("Topic {}", id),
                })
                .collect::<Vec<_>>();

            return Err(CyclicDependenciesError(names));
        }

        let idx = match &graph[scc[0]] {
            &Node::Handler(idx) => idx,
            _ => continue,
        };

        handlers.push(handlers_temp[idx].take().expect("Node appears in two SCCs"));
    }

    Ok(())
}
