use pico_core::{
    container::Container,
    database::Database,
    dyn_eq::DynEq,
    epoch::Epoch,
    key::Key,
    node::{Dependency, DerivedNode, DerivedNodeId, NodeKind},
    params::ParamId,
    storage::{Storage, StorageMut},
};

pub enum DidRecalculate {
    ReusedMemoizedValue,
    Recalculated,
}

pub fn memo<Db: Database>(
    db: &mut Db,
    node_id: DerivedNodeId,
    inner_fn: fn(&mut Db, ParamId) -> Box<dyn DynEq>,
) -> DidRecalculate {
    let current_epoch = db.current_epoch();
    let (time_updated, did_recalculate) = if db.storage().derived_nodes().contains_key(&node_id) {
        if any_dependency_changed(db, node_id, current_epoch) {
            let mut state = DidRecalculate::ReusedMemoizedValue;
            let mut alt_updated_time = None;
            let (value, dependencies, time_updated) =
                call_inner_fn_and_collect_dependencies(db, node_id.param_id, inner_fn);
            if let Some(node) = db.storage_mut().derived_nodes().get_mut(&node_id) {
                eprintln!(
                    "eq {} original updated {:?} new updated {:?}",
                    node.value == value,
                    node.time_updated,
                    time_updated
                );

                if node.value == value && (node.time_updated != time_updated) {
                    eprintln!("WRONG");
                    // panic!("asdf");
                }

                if node.value != value {
                    node.value = value;
                    node.time_updated = time_updated;
                    state = DidRecalculate::Recalculated;
                }
                // sum -> (left dependency) -> left -> (source dep) -> source
                //        time updated: t0     tU: t1
                node.dependencies = dependencies;
                node.time_verified = current_epoch;
                alt_updated_time = Some(node.time_updated);
            } else {
                db.storage_mut().derived_nodes().insert(
                    node_id,
                    DerivedNode {
                        time_verified: current_epoch,
                        time_updated,
                        dependencies,
                        inner_fn,
                        value,
                    },
                );
                state = DidRecalculate::Recalculated;
            }
            (
                if let Some(alt_time_updated) = alt_updated_time {
                    alt_time_updated
                } else {
                    time_updated
                },
                state,
            )
        } else {
            let node = db
                .storage_mut()
                .derived_nodes()
                .get_mut(&node_id)
                .expect("node should exist. This is indicative of a bug in Pico.");
            node.time_verified = current_epoch;
            (node.time_updated, DidRecalculate::ReusedMemoizedValue)
        }
    } else {
        let (value, dependencies, time_updated) =
            call_inner_fn_and_collect_dependencies(db, node_id.param_id, inner_fn);
        db.storage_mut().derived_nodes().insert(
            node_id,
            DerivedNode {
                time_verified: current_epoch,
                time_updated,
                dependencies,
                inner_fn,
                value,
            },
        );
        (time_updated, DidRecalculate::Recalculated)
    };
    register_dependency_in_parent_memoized_fn(
        db,
        NodeKind::Derived(node_id),
        time_updated,
        current_epoch,
    );
    did_recalculate
}

fn any_dependency_changed<Db: Database>(
    db: &mut Db,
    node_id: DerivedNodeId,
    current_epoch: Epoch,
) -> bool {
    let potentially_changed_dependencies = db
        .storage()
        .derived_nodes()
        .get(&node_id)
        .expect("node should exist. This is indicative of a bug in Pico.")
        .dependencies
        .iter()
        .filter_map(|dep| {
            if dep.time_verified_or_updated != current_epoch {
                Some(*dep)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    potentially_changed_dependencies
        .into_iter()
        .any(|dependency| match dependency.node_to {
            NodeKind::Source(key) => {
                source_node_changed_since(db, key, dependency.time_verified_or_updated)
            }
            NodeKind::Derived(dep_node_id) => {
                derived_node_changed_since(db, dep_node_id, dependency.time_verified_or_updated)
            }
        })
}

fn source_node_changed_since<Db: Database>(db: &Db, key: Key, since: Epoch) -> bool {
    match db.storage().source_nodes().get(&key) {
        Some(source) => source.time_updated > since,
        None => panic!("Source node not found. This indicates a bug in Pico."),
    }
}

fn derived_node_changed_since<Db: Database>(
    db: &mut Db,
    node_id: DerivedNodeId,
    dependency_time_verified: Epoch,
) -> bool {
    if !db.storage().params().contains_key(&node_id.param_id) {
        return true;
    }
    let inner_fn = if let Some(node) = db.storage().derived_nodes().get(&node_id) {
        if node.time_updated > dependency_time_verified {
            return true;
        }
        node.inner_fn
    } else {
        return true;
    };
    let did_recalculate = memo(db, node_id, inner_fn);
    matches!(did_recalculate, DidRecalculate::Recalculated)
}

fn call_inner_fn_and_collect_dependencies<Db: Database>(
    db: &mut Db,
    param_id: ParamId,
    inner_fn: fn(&mut Db, ParamId) -> Box<dyn DynEq>,
) -> (
    Box<dyn DynEq>,  /* value */
    Vec<Dependency>, /* dependencies */
    Epoch,           /* time_updated */
) {
    let (value, registered_dependencies) = with_dependency_tracking(db, param_id, inner_fn);
    let (dependencies, time_updated) = registered_dependencies.into_iter().fold(
        (vec![], Epoch::new()),
        |(mut deps, mut max_time_updated), (time_updated, dep)| {
            deps.push(dep);
            max_time_updated = std::cmp::max(max_time_updated, time_updated);
            (deps, max_time_updated)
        },
    );
    (value, dependencies, time_updated)
}

pub fn register_dependency_in_parent_memoized_fn<Db: Database>(
    db: &mut Db,
    node: NodeKind,
    time_updated: Epoch,
    current_epoch: Epoch,
) {
    if let Some(dependencies) = db.storage_mut().dependency_stack().last_mut() {
        dependencies.push((
            time_updated,
            Dependency {
                node_to: node,
                time_verified_or_updated: current_epoch,
            },
        ));
    } else {
        // Dependency stack is empty for the outermost memoized function.
        // We don't need to register dependencies for it.
    }
}

fn with_dependency_tracking<Db>(
    db: &mut Db,
    param_id: ParamId,
    inner_fn: fn(&mut Db, ParamId) -> Box<dyn DynEq>,
) -> (Box<dyn DynEq>, Vec<(Epoch, Dependency)>)
where
    Db: Database,
{
    db.storage_mut().dependency_stack().push(vec![]);
    let value = inner_fn(db, param_id);
    let registered_dependencies = db
        .storage_mut()
        .dependency_stack()
        .pop()
        .expect("Dependency stack should not be empty. This indicates a bug in Pico.");
    (value, registered_dependencies)
}
