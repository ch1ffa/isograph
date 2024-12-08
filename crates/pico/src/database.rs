use tracing::debug;

use crate::{
    dyn_eq::DynEq,
    hashmap_storage::HashMapStorage,
    node::{Dependency, DerivedNode, NodeId, SourceNode},
    params::ParamId,
};

#[derive(Debug)]
pub struct Database {
    pub current_epoch: usize,
    pub dependency_stack: Vec<Vec<(usize, Dependency)>>,
    pub nodes: HashMapStorage<NodeId>,
    pub params: HashMapStorage<ParamId>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            current_epoch: 0,
            dependency_stack: vec![],
            nodes: HashMapStorage::new(),
            params: HashMapStorage::new(),
        }
    }

    pub fn memo(
        &mut self,
        static_key: &'static str,
        param_id: ParamId,
        inner_fn: impl Fn(&mut Database, ParamId) -> Box<dyn DynEq> + 'static,
    ) -> NodeId {
        let node_id = NodeId::new::<DerivedNode>(static_key, param_id);
        let time_calculated = if self.nodes.contains_key(&node_id) {
            if self.any_dependency_changed(&node_id) {
                debug!("found node for {static_key}, dependency changed");
                let (value, dependencies, time_calculated) =
                    self.call_inner_fn_and_collect_dependencies(param_id, &inner_fn);
                let node = self
                    .nodes
                    .get_mut::<DerivedNode>(&node_id)
                    .expect("node should exist");
                if node.value != value {
                    debug!("value changed for node {static_key}");
                    node.value = value
                } else {
                    debug!("value not changed for node {static_key}");
                }
                node.dependencies = dependencies;
                node.time_calculated = time_calculated;
                node.time_verified = self.current_epoch;
                time_calculated
            } else {
                debug!("found node for {static_key}, no dependency changed");
                self.nodes
                    .get::<DerivedNode>(&node_id)
                    .expect("node should exist")
                    .time_calculated
            }
        } else {
            debug!("node not found for {static_key}, creating");
            let (value, dependencies, time_calculated) =
                self.call_inner_fn_and_collect_dependencies(param_id, &inner_fn);
            let node = DerivedNode {
                value,
                time_verified: self.current_epoch,
                time_calculated,
                dependencies,
                calculate: Box::new(move |db| inner_fn(db, param_id)),
            };
            self.nodes.insert(node_id, node);
            time_calculated
        };

        if let Some(dependencies) = self.dependency_stack.last_mut() {
            dependencies.push((
                time_calculated,
                Dependency {
                    node_to: node_id,
                    time_verified_or_calculated: self.current_epoch,
                },
            ));
        }
        node_id
    }

    fn any_dependency_changed(&mut self, node_id: &NodeId) -> bool {
        let mut has_changed = false;
        let child_nodes = self
            .nodes
            .get::<DerivedNode>(node_id)
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
        for (child_node_id, time_verified) in child_nodes {
            if child_node_id.of_type::<DerivedNode>() {
                let child_node = self
                    .nodes
                    .remove::<DerivedNode>(&child_node_id)
                    .expect("node should exist");
                let new_value = (child_node.calculate)(self);
                if child_node.value != new_value {
                    has_changed = true;
                }
                self.nodes.insert(child_node_id, child_node);
                if has_changed {
                    break;
                }
            } else {
                let time_calculated = self
                    .nodes
                    .get::<SourceNode>(&child_node_id)
                    .expect("node should exist")
                    .time_calculated;
                if time_calculated > time_verified {
                    has_changed = true;
                    break;
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
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}
