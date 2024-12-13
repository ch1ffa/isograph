use std::collections::HashMap;

use tracing::debug;

use crate::{
    dyn_eq::DynEq,
    hashmap_storage::HashMapStorage,
    node::{Dependency, DerivedNode, NodeId, NodeKind, SourceNode},
    params::ParamId,
};

#[derive(Debug, Default)]
pub struct Database {
    pub current_epoch: usize,
    pub dependency_stack: Vec<Vec<(usize, Dependency)>>,
    pub nodes: HashMap<NodeId, DerivedNode>,
    pub sources: HashMap<NodeId, SourceNode>,
    pub params: HashMapStorage<ParamId>,
    pub values: HashMap<NodeId, Box<dyn DynEq>>,
}

impl Database {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::map_entry)]
    pub fn memo(
        &mut self,
        static_key: &'static str,
        param_id: ParamId,
        inner_fn: fn(&mut Database, ParamId) -> Box<dyn DynEq>,
    ) -> NodeId {
        let node_id = NodeId::derived(static_key, param_id);
        let time_calculated = if self.nodes.contains_key(&node_id) {
            if self.any_dependency_changed(&node_id) {
                debug!("node \"{static_key}\": dependency changed");
                let (new_value, dependencies, time_calculated) =
                    self.call_inner_fn_and_collect_dependencies(param_id, inner_fn);
                let node = self.nodes.get_mut(&node_id).expect("node should exist");
                let value = self.values.get_mut(&node_id).expect("value should exist");
                if *value != new_value {
                    debug!("node \"{static_key}\": value changed");
                    *value = new_value
                } else {
                    debug!("node \"{static_key}\": value not changed");
                }
                node.dependencies = dependencies;
                node.time_calculated = time_calculated;
                node.time_verified = self.current_epoch;
                time_calculated
            } else {
                debug!("node \"{static_key}\": no dependency changed");
                self.nodes
                    .get(&node_id)
                    .expect("node should exist")
                    .time_calculated
            }
        } else {
            debug!("new node \"{static_key}\"");
            let (value, dependencies, time_calculated) =
                self.call_inner_fn_and_collect_dependencies(param_id, inner_fn);
            self.nodes.insert(
                node_id,
                DerivedNode {
                    time_verified: self.current_epoch,
                    time_calculated,
                    dependencies,
                    inner_fn,
                },
            );
            self.values.insert(node_id, value);
            time_calculated
        };

        self.register_dependency(node_id, time_calculated);
        node_id
    }

    fn any_dependency_changed(&mut self, node_id: &NodeId) -> bool {
        let mut has_changed = false;
        let maybe_changed = self
            .nodes
            .get(node_id)
            .expect("node should exist")
            .dependencies
            .iter()
            .filter_map(|dependency| {
                if dependency.time_verified_or_calculated != self.current_epoch {
                    Some((dependency.node_to, dependency.time_verified_or_calculated))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for (node_id, time_verified) in maybe_changed {
            match node_id.kind {
                NodeKind::Source => {
                    let time_calculated = self
                        .sources
                        .get(&node_id)
                        .expect("node should exist")
                        .time_calculated;
                    if time_calculated > time_verified {
                        has_changed = true;
                        break;
                    }
                }
                NodeKind::Derived => {
                    let value = (self
                        .nodes
                        .get(&node_id)
                        .expect("node should exist")
                        .inner_fn)(self, node_id.param_id);
                    if *self.values.get(&node_id).expect("value should exist") != value {
                        has_changed = true;
                        break;
                    }
                }
            }
        }
        has_changed
    }

    fn call_inner_fn_and_collect_dependencies(
        &mut self,
        param_id: ParamId,
        inner_fn: impl Fn(&mut Database, ParamId) -> Box<dyn DynEq>,
    ) -> (
        Box<dyn DynEq>,  /* value */
        Vec<Dependency>, /* dependencies */
        usize,           /* time_calculated */
    ) {
        self.dependency_stack.push(vec![]);
        let value = inner_fn(self, param_id);
        let registred_dependencies = self
            .dependency_stack
            .pop()
            .expect("dependency stack to not be empty");

        let (dependencies, time_calculated) = registred_dependencies.into_iter().fold(
            (vec![], 0),
            |(mut deps, mut max_time_calculated), (time_calculated, dep)| {
                deps.push(dep);
                max_time_calculated = std::cmp::max(max_time_calculated, time_calculated);
                (deps, max_time_calculated)
            },
        );
        (value, dependencies, time_calculated)
    }

    pub fn register_dependency(&mut self, node_id: NodeId, time_calculated: usize) {
        if let Some(dependencies) = self.dependency_stack.last_mut() {
            dependencies.push((
                time_calculated,
                Dependency {
                    node_to: node_id,
                    time_verified_or_calculated: self.current_epoch,
                },
            ));
        }
    }
}
