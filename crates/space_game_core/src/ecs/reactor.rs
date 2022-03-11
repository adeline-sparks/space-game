//! `Reactor` and related types.

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

/// `Event` which is fired at init time, which [`Handler`]s can use to initialize their state.
#[derive(Debug)]
pub struct InitEvent;
impl Event for InitEvent {}

/// Stores a set of [`Handler`]s and executes them in response to [`Event`]s.
/// 
/// `Handler`s are able to emit their own `Events`, which are dispatched 
/// similarly after the initial `Event`. If the `Handler` returns an error while 
/// handling any `Event`, it is logged but dispatch of that `Event` continues.
pub struct Reactor(HashMap<EventId, Vec<Handler>>);

impl Reactor {
    /// Begin constructing a `Reactor` via [`ReactorBuilder`].
    pub fn builder() -> ReactorBuilder {
        ReactorBuilder::default()
    }

    /// Create a fresh [`StateContainer`] for use with this `Reactor`. 
    ///
    /// This will automatically dispatch an [`InitEvent`] so that handlers
    /// can initialize their state.
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

    /// Dispatch an event to all handlers and update the `states`.
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
                        error!("Handler '{}' failed while handling {:?}: {}", h, event, err);
                    }
                }
            }
        }
    }
}

/// Builder type for [`Reactor`].
#[derive(Default)]
pub struct ReactorBuilder(Vec<Handler>);

/// Errors which can occur while building the reactor.
#[derive(Error, Debug)]
pub enum BuildReactorError {
    /// Indicates that the handlers for the given [`EventId`] have a circular dependency.
    #[error("While processing {0}: {1}")]
    Cycle(EventId, #[source] CyclicDependenciesError),
}

impl ReactorBuilder {
    /// Add a handler function to the ReactorBuilder. See [`HandlerFn`].
    pub fn add<E: Event, Args>(mut self, f: impl HandlerFn<E, Args>) -> Self {
        self.0.push(f.into_handler());
        self
    }

    /// Build the [`Reactor`].
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

/// Indicates that a cyclic dependency was found. Each `String` describes a participant in the cycle.
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

/// Re-arranges `handlers` in-place to build a topographical order suitable for dispatch.
fn sort_handlers_by_execution_order(
    handlers: &mut Vec<Handler>,
) -> Result<(), CyclicDependenciesError> {
    /// Node type for the dependency graph.
    enum Node {
        /// Node represents the handler at the given index in `handlers`.
        Handler(usize),
        /// Node represents a `State`.
        State(StateId),
        /// Node represents a `Topic`.
        Topic(TopicId),
    }

    // First, we construct the nodes of the graph. As we go, populate `HashMap`s for fast 
    // retrieval of nodes their ID.
    let mut graph = DiGraph::<Node, ()>::new();
    let mut handler_nodes = Vec::new();
    let mut state_nodes = HashMap::new();
    let mut topic_nodes = HashMap::new();

    for (idx, handler) in handlers.iter().enumerate() {
        // Build a node for this handler.
        handler_nodes.push(graph.add_node(Node::Handler(idx)));

        // Check each dependency and build nodes if they refer to things
        // we don't already have nodes for.
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
    }

    // Next, populate incoming and outgoing edges for each handler. Edges point from dependee to dependency.
    for &handler_node in &handler_nodes {
        let handler = match &graph[handler_node] {
            &Node::Handler(idx) => &handlers[idx],
            _ => panic!("Non-handler found at handler_node index"),
        };

        for dep in handler.dependencies() {
            match dep {
                Dependency::ReadState(id) => {
                    graph.add_edge(handler_node, state_nodes[id], ());
                }
                Dependency::ReadStateDelayed(id) | Dependency::WriteState(id) => {
                    graph.add_edge(state_nodes[id], handler_node, ());
                }
                Dependency::SubscribeTopic(id) => {
                    graph.add_edge(handler_node, topic_nodes[id], ());
                }
                Dependency::PublishTopic(id) => {
                    graph.add_edge(topic_nodes[id], handler_node, ());
                }
            }
        }
    }

    // Find strongly connected components for the graph in reverse topological order.
    let sccs_rev_topo = kosaraju_scc(&graph);

    // Drain the handlers vec into a temporary we can take from.
    let mut handlers_temp = handlers.drain(..).map(Some).collect::<Vec<_>>();

    // Scan each component.
    for scc in sccs_rev_topo {
        // If there are multiple nodes in the strongly connected component, they form a cycle.
        if scc.len() > 1 {
            // Return an error describing the cycle.
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

        // Append handlers to our output by taking them from the temporary storage.
        if let &Node::Handler(idx) = &graph[scc[0]] {
            handlers.push(handlers_temp[idx].take().expect("Node appears in two SCCs"));
        }
    }
 
    Ok(())
}
