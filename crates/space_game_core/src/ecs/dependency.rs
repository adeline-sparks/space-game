use std::collections::{hash_map, HashMap, HashSet};
use std::slice;

use super::state::StateId;
use super::topic::TopicId;

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub enum Dependency {
    ReadState(StateId),
    ReadStateDelayed(StateId),
    WriteState(StateId),
    SubscribeTopic(TopicId),
    PublishTopic(TopicId),
}

pub enum ExecutionOrderError {
    WriteConflict(StateId, usize, usize),
    Cyclic(Vec<usize>),
}

impl ExecutionOrderError {
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
