use common_lang_types::{
    ScalarSelectableName, SelectableAlias, SelectableNameOrAlias, ServerObjectSelectableName,
    VariableName, WithLocation, WithSpan,
};
use resolve_position::PositionResolutionPath;
use resolve_position_macros::ResolvePosition;

use crate::{
    ClientFieldDeclarationPath, ClientPointerDeclarationPath, IsographResolvedNode,
    NonConstantValue, ObjectSelectionDirectiveSet, ScalarSelectionDirectiveSet,
    SelectionFieldArgument, SelectionType,
};

pub type UnvalidatedSelection = SelectionTypeContainingSelections<(), ()>;

pub type UnvalidatedScalarFieldSelection = ScalarSelection<()>;

pub type SelectionTypeContainingSelections<TScalarField, TLinkedField> =
    SelectionType<ScalarSelection<TScalarField>, ObjectSelection<TScalarField, TLinkedField>>;

impl<TScalarField, TLinkedField> SelectionTypeContainingSelections<TScalarField, TLinkedField> {
    pub fn name_or_alias(&self) -> WithLocation<SelectableNameOrAlias> {
        match self {
            SelectionTypeContainingSelections::Scalar(scalar_field) => scalar_field.name_or_alias(),
            SelectionTypeContainingSelections::Object(linked_field) => linked_field.name_or_alias(),
        }
    }

    pub fn variables<'a>(&'a self) -> impl Iterator<Item = VariableName> + 'a {
        let get_variable = |x: &'a WithLocation<SelectionFieldArgument>| match x.item.value.item {
            NonConstantValue::Variable(v) => Some(v),
            _ => None,
        };
        match self {
            SelectionTypeContainingSelections::Scalar(scalar_field) => {
                scalar_field.arguments.iter().flat_map(get_variable)
            }
            SelectionTypeContainingSelections::Object(linked_field) => {
                linked_field.arguments.iter().flat_map(get_variable)
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, ResolvePosition)]
#[resolve_position(parent_type=SelectionParentType<'a>, resolved_node=IsographResolvedNode<'a>, self_type_generics=<()>)]
// TODO remove type parameter
pub struct ScalarSelection<TScalarField> {
    // TODO make this WithSpan instead of WithLocation
    pub name: WithLocation<ScalarSelectableName>,
    // TODO make this WithSpan instead of WithLocation
    pub reader_alias: Option<WithLocation<SelectableAlias>>,
    pub associated_data: TScalarField,
    // TODO make this WithSpan instead of WithLocation
    pub arguments: Vec<WithLocation<SelectionFieldArgument>>,
    pub scalar_selection_directive_set: ScalarSelectionDirectiveSet,
}

pub type ScalarSelectionPath<'a> =
    PositionResolutionPath<&'a ScalarSelection<()>, SelectionParentType<'a>>;

impl<TScalarField> ScalarSelection<TScalarField> {
    pub fn name_or_alias(&self) -> WithLocation<SelectableNameOrAlias> {
        self.reader_alias
            .map(|item| item.map(SelectableNameOrAlias::from))
            .unwrap_or_else(|| self.name.map(SelectableNameOrAlias::from))
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, ResolvePosition)]
#[resolve_position(parent_type=SelectionParentType<'a>, resolved_node=IsographResolvedNode<'a>, self_type_generics=<(), ()>)]
// TODO remove the type parameters
pub struct ObjectSelection<TScalar, TLinked> {
    // TODO make this WithSpan instead of WithLocation
    pub name: WithLocation<ServerObjectSelectableName>,
    // TODO make this WithSpan instead of WithLocation
    pub reader_alias: Option<WithLocation<SelectableAlias>>,
    pub associated_data: TLinked,
    #[resolve_field]
    pub selection_set: Vec<WithSpan<SelectionTypeContainingSelections<TScalar, TLinked>>>,
    // TODO make this WithSpan instead of WithLocation
    pub arguments: Vec<WithLocation<SelectionFieldArgument>>,
    pub object_selection_directive_set: ObjectSelectionDirectiveSet,
}

pub type ObjectSelectionPath<'a> =
    PositionResolutionPath<&'a ObjectSelection<(), ()>, SelectionParentType<'a>>;

#[derive(Debug)]
pub enum SelectionParentType<'a> {
    ObjectSelection(Box<ObjectSelectionPath<'a>>),
    ClientFieldDeclaration(ClientFieldDeclarationPath<'a>),
    ClientPointerDeclaration(ClientPointerDeclarationPath<'a>),
}

impl<TScalarField, TLinkedField> ObjectSelection<TScalarField, TLinkedField> {
    pub fn name_or_alias(&self) -> WithLocation<SelectableNameOrAlias> {
        self.reader_alias
            .map(|item| item.map(SelectableNameOrAlias::from))
            .unwrap_or_else(|| self.name.map(SelectableNameOrAlias::from))
    }
}
