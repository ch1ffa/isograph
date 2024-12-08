use std::fmt;
use std::{any::TypeId, hash::Hash};

use crate::params::ParamId;
use crate::{database::Database, dyn_eq::DynEq};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    type_id: TypeId,
    key: &'static str,
    param_id: ParamId,
}

impl NodeId {
    pub fn new<T: 'static>(key: &'static str, param_id: ParamId) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            key,
            param_id,
        }
    }

    pub fn of_type<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }
}

pub struct DerivedNode {
    pub value: Box<dyn DynEq>,
    pub time_verified: usize,
    pub time_calculated: usize,
    pub dependencies: Vec<Dependency>,
    pub calculate: Box<dyn Fn(&mut Database) -> Box<dyn DynEq>>,
}

impl fmt::Debug for DerivedNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DerivedNode")
            .field("time_verified", &self.time_verified)
            .field("time_calculated", &self.time_calculated)
            .field("dependencies", &self.dependencies)
            .finish()
    }
}

pub struct SourceNode {
    pub value: Box<dyn DynEq>,
    pub time_calculated: usize,
}

impl fmt::Debug for SourceNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SourceNode")
            .field("time_calculated", &self.time_calculated)
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dependency {
    pub node_to: NodeId,
    pub time_verified_or_calculated: usize,
}
