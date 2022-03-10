//! Contains types and functions for tracking dependencies on `State`s and `Topic`s, and using this information to run Handlers in the proper order.

use std::collections::{hash_map, HashMap, HashSet};
use std::slice;

use super::state::StateId;
use super::topic::TopicId;

/// Represents a dependency that a `Handler` can have.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum Dependency {
    /// Dependency on reading from a `State`.
    ReadState(StateId),
    /// Dependency on reading from a `State` with one cycle delay.
    ReadStateDelayed(StateId),
    /// Dependency on writing to a `State`.
    WriteState(StateId),
    /// Dependency on subscribing to a `Topic`.
    SubscribeTopic(TopicId),
    /// Dependency on publishing to a `Topic`.
    PublishTopic(TopicId),
}

impl Dependency {
    /// Returns the `StateId` this dependency is related to, or None if this dependency is not related to a `State`.
    pub fn state_id(&self) -> Option<&StateId> {
        match self {
            Dependency::ReadState(id)
            | Dependency::ReadStateDelayed(id)
            | Dependency::WriteState(id) => Some(id),
            Dependency::SubscribeTopic(_) | Dependency::PublishTopic(_) => None,
        }
    }
}

/// An error found while determing the `Handler` execution order using dependencies.
pub enum ExecutionOrderError {
    /// The two `Handler` indices both write to the given `StateId`.
    WriteConflict(StateId, usize, usize),
    /// The given `Handler` indices form a dependency cycle.
    Cyclic(Vec<usize>),
}

impl ExecutionOrderError {
    // TODO: ditch this, let's just pass Handlers in and retrieve their name when needed.
    pub fn error_message<'a>(&self, get_name: impl Fn(usize) -> String) -> String {
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

/// TODO
pub fn execution_order(all_deps: &[&[Dependency]]) -> Result<Vec<usize>, Vec<ExecutionOrderError>> {
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
                    if let Some(writer) = writers.get(tid) {
                        (slice::from_ref(&idx), *writer)
                    } else {
                        continue;
                    }
                }
                Dependency::ReadStateDelayed(tid) => {
                    if let Some(writer) = writers.get(tid) {
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

    /// State for our depth first traversal.
    struct Env<'s> {
        /// Map of parent indice to child indices.
        children: &'s HashMap<usize, Vec<usize>>,
        /// Map of unvisited indices.
        unvisited: HashSet<usize>,
        /// Set of indices we are currently visiting.
        pending: HashSet<usize>,
        /// Stack of indices we are currently visiting, in the order they were visited.
        pending_stack: Vec<usize>,
        /// Indices output in depth first order.
        result: Vec<usize>,
        /// Errors found during traversal.
        errors: &'s mut Vec<ExecutionOrderError>,
    }

    impl Env<'_> {
        /// Recursive depth first traversal which visits all children, then outputs the curent index to the result.
        fn visit(&mut self, idx: usize) {
            // If this index is in the pending set, we reached it while traversing its children. Record an error and return immediately to avoid an infinite loop.
            if self.pending.contains(&idx) {
                let mut cycle = self.pending_stack.clone();
                cycle.reverse();
                self.errors.push(ExecutionOrderError::Cyclic(cycle));
                return;
            }

            // Mark this node as visited. If it was already marked, exit.
            if !self.unvisited.remove(&idx) {
                return;
            }

            // Add this node to the pending set and stack.
            assert!(!self.pending.insert(idx));
            self.pending_stack.push(idx);

            // Visit all of our children.
            for &child_idx in self.children.get(&idx).into_iter().flatten() {
                self.visit(child_idx);
            }

            // Remove thsi node from the pending set and stack.
            assert!(self.pending.remove(&idx));
            self.pending_stack.pop();

            // Append this node to the otuput.
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

    // As long as we have unvisited nodes, grab one and visit it.
    while let Some(&idx) = state.unvisited.iter().next() {
        state.visit(idx);
    }

    // Once all nodes are visited, the resulting output is our execution order.
    Ok(state.result)
}
