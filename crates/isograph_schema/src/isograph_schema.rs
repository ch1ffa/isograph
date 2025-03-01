use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    marker::PhantomData,
};

use common_lang_types::{
    ClientPointerFieldName, DescriptionValue, GraphQLInterfaceTypeName, GraphQLScalarTypeName,
    IsographObjectTypeName, JavascriptName, ObjectTypeAndFieldName, SelectableFieldName,
    UnvalidatedTypeName, WithLocation, WithSpan,
};
use graphql_lang_types::{
    GraphQLConstantValue, GraphQLDirective, GraphQLFieldDefinition,
    GraphQLInputObjectTypeDefinition, GraphQLInterfaceTypeDefinition, GraphQLObjectTypeDefinition,
};
use intern::string_key::Intern;
use isograph_lang_types::{
    ArgumentKeyAndValue, ClientFieldId, ClientPointerId, DefinitionLocation,
    SelectableServerFieldId, SelectionType, ServerFieldId, ServerFieldSelection, ServerObjectId,
    ServerScalarId, ServerStrongIdFieldId, TypeAnnotation, VariableDefinition,
};
use lazy_static::lazy_static;

use crate::{
    refetch_strategy::RefetchStrategy, schema_validation_state::SchemaValidationState,
    ClientFieldVariant, NormalizationKey, OutputFormat, ServerFieldTypeAssociatedData,
    ValidatedClientField, ValidatedClientPointer,
};

lazy_static! {
    pub static ref ID_GRAPHQL_TYPE: GraphQLScalarTypeName = "ID".intern().into();
}

#[derive(Debug, Clone)]
pub struct RootOperationName(pub String);

/// The in-memory representation of a schema.
///
/// The TSchemaValidationState type param varies based on how far along in the
/// validation pipeline the schema instance is, i.e. validating the schema means
/// consuming an instance and creating a new instance with another
/// TSchemaValidationState.
///
/// The TOutputFormat type param will stay constant as the schema is validated.
///
/// Invariant: a schema is append-only, because pointers into the Schema are in the
/// form of newtype wrappers around u32 indexes (e.g. FieldId, etc.) As a result,
/// the schema does not support removing items.
#[derive(Debug)]
pub struct Schema<TSchemaValidationState: SchemaValidationState, TOutputFormat: OutputFormat> {
    pub server_fields: Vec<
        SchemaServerField<
            TSchemaValidationState::ServerFieldTypeAssociatedData,
            TSchemaValidationState::VariableDefinitionInnerType,
            TOutputFormat,
        >,
    >,
    pub client_types: SelectionTypes<
        TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
        TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
        TSchemaValidationState::VariableDefinitionInnerType,
        TOutputFormat,
    >,
    // TODO consider whether this belongs here. It could just be a free variable.
    pub entrypoints: TSchemaValidationState::Entrypoint,
    pub server_field_data: ServerFieldData<TOutputFormat>,

    /// These are root types like Query, Mutation, Subscription
    pub fetchable_types: BTreeMap<ServerObjectId, RootOperationName>,
}

type SelectionTypes<
    TSelectionTypeSelectionScalarFieldAssociatedData,
    TSelectionTypeSelectionLinkedFieldAssociatedData,
    TClientFieldVariableDefinitionAssociatedData,
    TOutputFormat,
> = Vec<
    SelectionType<
        ClientField<
            TSelectionTypeSelectionScalarFieldAssociatedData,
            TSelectionTypeSelectionLinkedFieldAssociatedData,
            TClientFieldVariableDefinitionAssociatedData,
            TOutputFormat,
        >,
        ClientPointer<
            TSelectionTypeSelectionScalarFieldAssociatedData,
            TSelectionTypeSelectionLinkedFieldAssociatedData,
            TClientFieldVariableDefinitionAssociatedData,
            TOutputFormat,
        >,
    >,
>;

impl<TSchemaValidationState: SchemaValidationState, TOutputFormat: OutputFormat>
    Schema<TSchemaValidationState, TOutputFormat>
{
    /// This is a smell, and we should refactor away from it, or all schema's
    /// should have a root type.
    pub fn query_id(&self) -> ServerObjectId {
        *self
            .fetchable_types
            .iter()
            .find(|(_, root_operation_name)| root_operation_name.0 == "query")
            .expect("Expected query to be found")
            .0
    }

    pub fn find_mutation(&self) -> Option<(&ServerObjectId, &RootOperationName)> {
        self.fetchable_types
            .iter()
            .find(|(_, root_operation_name)| root_operation_name.0 == "mutation")
    }

    pub fn find_query(&self) -> Option<(&ServerObjectId, &RootOperationName)> {
        self.fetchable_types
            .iter()
            .find(|(_, root_operation_name)| root_operation_name.0 == "query")
    }
}

pub type LinkedType<
    'a,
    ServerFieldTypeAssociatedData,
    SelectionTypeSelectionScalarFieldAssociatedData,
    SelectionTypeSelectionLinkedFieldAssociatedData,
    VariableDefinitionInnerType,
    TOutputFormat,
> = DefinitionLocation<
    &'a SchemaServerField<
        ServerFieldTypeAssociatedData,
        VariableDefinitionInnerType,
        TOutputFormat,
    >,
    &'a ClientPointer<
        SelectionTypeSelectionScalarFieldAssociatedData,
        SelectionTypeSelectionLinkedFieldAssociatedData,
        VariableDefinitionInnerType,
        TOutputFormat,
    >,
>;

pub type SelectionTypeId = SelectionType<ClientFieldId, ClientPointerId>;

pub type ValidatedSelectionType<'a, TOutputFormat> = SelectionType<
    &'a ValidatedClientField<TOutputFormat>,
    &'a ValidatedClientPointer<TOutputFormat>,
>;

#[derive(Debug)]
pub struct ServerFieldData<TOutputFormat: OutputFormat> {
    pub server_objects: Vec<SchemaObject<TOutputFormat>>,
    pub server_scalars: Vec<SchemaScalar<TOutputFormat>>,
    pub defined_types: HashMap<UnvalidatedTypeName, SelectableServerFieldId>,

    // Well known types
    pub id_type_id: ServerScalarId,
    pub string_type_id: ServerScalarId,
    pub float_type_id: ServerScalarId,
    pub boolean_type_id: ServerScalarId,
    pub int_type_id: ServerScalarId,
    // TODO restructure UnionTypeAnnotation to not have a nullable field, but to instead
    // include null in its variants.
    pub null_type_id: ServerScalarId,
}

impl<TSchemaValidationState: SchemaValidationState, TOutputFormat: OutputFormat>
    Schema<TSchemaValidationState, TOutputFormat>
{
    /// Get a reference to a given server field by its id.
    pub fn server_field(
        &self,
        server_field_id: ServerFieldId,
    ) -> &SchemaServerField<
        TSchemaValidationState::ServerFieldTypeAssociatedData,
        TSchemaValidationState::VariableDefinitionInnerType,
        TOutputFormat,
    > {
        &self.server_fields[server_field_id.as_usize()]
    }

    /// Get a reference to a given client field by its id.
    pub fn client_field(
        &self,
        client_field_id: ClientFieldId,
    ) -> &ClientField<
        TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
        TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
        TSchemaValidationState::VariableDefinitionInnerType,
        TOutputFormat,
    > {
        match &self.client_types[client_field_id.as_usize()] {
            SelectionType::Scalar(client_field) => client_field,
            SelectionType::Object(_) => panic!(
                "encountered ClientPointer under ClientFieldId. \
                This is indicative of a bug in Isograph."
            ),
        }
    }

    pub fn linked_type(
        &self,
        field_id: DefinitionLocation<ServerFieldId, ClientPointerId>,
    ) -> LinkedType<
        TSchemaValidationState::ServerFieldTypeAssociatedData,
        TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
        TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
        TSchemaValidationState::VariableDefinitionInnerType,
        TOutputFormat,
    > {
        match field_id {
            DefinitionLocation::Server(server_field_id) => {
                DefinitionLocation::Server(self.server_field(server_field_id))
            }
            DefinitionLocation::Client(client_pointer_id) => {
                DefinitionLocation::Client(self.client_pointer(client_pointer_id))
            }
        }
    }

    /// Get a reference to a given client pointer by its id.
    pub fn client_pointer(
        &self,
        client_pointer_id: ClientPointerId,
    ) -> &ClientPointer<
        TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
        TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
        TSchemaValidationState::VariableDefinitionInnerType,
        TOutputFormat,
    > {
        match &self.client_types[client_pointer_id.as_usize()] {
            SelectionType::Object(client_pointer) => client_pointer,
            SelectionType::Scalar(_) => panic!(
                "encountered ClientField under ClientPointerId. \
                This is indicative of a bug in Isograph."
            ),
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn client_type(
        &self,
        client_type_id: SelectionTypeId,
    ) -> SelectionType<
        &ClientField<
            TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
            TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
            TSchemaValidationState::VariableDefinitionInnerType,
            TOutputFormat,
        >,
        &ClientPointer<
            TSchemaValidationState::SelectionTypeSelectionScalarFieldAssociatedData,
            TSchemaValidationState::SelectionTypeSelectionLinkedFieldAssociatedData,
            TSchemaValidationState::VariableDefinitionInnerType,
            TOutputFormat,
        >,
    > {
        match client_type_id {
            SelectionType::Scalar(client_field_id) => {
                SelectionType::Scalar(self.client_field(client_field_id))
            }
            SelectionType::Object(client_pointer_id) => {
                SelectionType::Object(self.client_pointer(client_pointer_id))
            }
        }
    }
}

impl<
        TObjectFieldAssociatedData: Clone + Ord + Copy + Debug,
        TScalarFieldAssociatedData: Clone + Ord + Copy + Debug,
        TSchemaValidationState: SchemaValidationState<
            ServerFieldTypeAssociatedData = SelectionType<
                TypeAnnotation<TScalarFieldAssociatedData>,
                ServerFieldTypeAssociatedData<TypeAnnotation<TObjectFieldAssociatedData>>,
            >,
        >,
        TOutputFormat: OutputFormat,
    > Schema<TSchemaValidationState, TOutputFormat>
{
    // This should not be this complicated!
    /// Get a reference to a given id field by its id.
    pub fn id_field<
        TError: Debug,
        TIdFieldAssociatedData: TryFrom<TScalarFieldAssociatedData, Error = TError> + Copy + Debug,
    >(
        &self,
        id_field_id: ServerStrongIdFieldId,
    ) -> SchemaIdField<TIdFieldAssociatedData> {
        let field_id = id_field_id.into();

        let field = self
            .server_field(field_id)
            .and_then(|e| match e {
                SelectionType::Object(_) => panic!(
                    "We had an id field, it should be scalar. This indicates a bug in Isograph.",
                ),
                SelectionType::Scalar(e) => e.inner_non_null().try_into(),
            })
            .expect(
                // N.B. this expect should never be triggered. This is only because server_field
                // does not have a .map method. TODO implement .map
                "We had an id field, the type annotation should be named. \
                    This indicates a bug in Isograph.",
            );

        field.try_into().expect(
            "We had an id field, no arguments should exist. This indicates a bug in Isograph.",
        )
    }
}

impl<TOutputFormat: OutputFormat> ServerFieldData<TOutputFormat> {
    /// Get a reference to a given scalar type by its id.
    pub fn scalar(&self, scalar_id: ServerScalarId) -> &SchemaScalar<TOutputFormat> {
        &self.server_scalars[scalar_id.as_usize()]
    }

    pub fn lookup_unvalidated_type(
        &self,
        type_id: SelectableServerFieldId,
    ) -> SchemaType<TOutputFormat> {
        match type_id {
            SelectableServerFieldId::Object(id) => {
                SchemaType::Object(self.server_objects.get(id.as_usize()).unwrap())
            }
            SelectableServerFieldId::Scalar(id) => {
                SchemaType::Scalar(self.server_scalars.get(id.as_usize()).unwrap())
            }
        }
    }

    /// Get a reference to a given object type by its id.
    pub fn object(&self, object_id: ServerObjectId) -> &SchemaObject<TOutputFormat> {
        &self.server_objects[object_id.as_usize()]
    }

    /// Get a mutable reference to a given object type by its id.
    pub fn object_mut(&mut self, object_id: ServerObjectId) -> &mut SchemaObject<TOutputFormat> {
        &mut self.server_objects[object_id.as_usize()]
    }
}

pub type SchemaType<'a, TOutputFormat> =
    SelectionType<&'a SchemaScalar<TOutputFormat>, &'a SchemaObject<TOutputFormat>>;

pub fn get_name<TOutputFormat: OutputFormat>(
    schema_type: SchemaType<'_, TOutputFormat>,
) -> UnvalidatedTypeName {
    match schema_type {
        SelectionType::Object(object) => object.name.into(),
        SelectionType::Scalar(scalar) => scalar.name.item.into(),
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct IsographObjectTypeDefinition {
    pub description: Option<WithSpan<DescriptionValue>>,
    pub name: WithLocation<IsographObjectTypeName>,
    // maybe this should be Vec<WithSpan<IsographObjectTypeName>>>
    pub interfaces: Vec<WithLocation<GraphQLInterfaceTypeName>>,
    /// Directives that we don't know about. Maybe this should be validated to be
    /// empty, or not exist.
    pub directives: Vec<GraphQLDirective<GraphQLConstantValue>>,
    // TODO the spans of these fields are wrong
    // TODO use a shared field type
    pub fields: Vec<WithLocation<GraphQLFieldDefinition>>,
}

impl From<GraphQLObjectTypeDefinition> for IsographObjectTypeDefinition {
    fn from(object_type_definition: GraphQLObjectTypeDefinition) -> Self {
        IsographObjectTypeDefinition {
            description: object_type_definition.description,
            name: object_type_definition.name.map(|x| x.into()),
            interfaces: object_type_definition.interfaces,
            directives: object_type_definition.directives,
            fields: object_type_definition.fields,
        }
    }
}

impl From<GraphQLInterfaceTypeDefinition> for IsographObjectTypeDefinition {
    fn from(value: GraphQLInterfaceTypeDefinition) -> Self {
        Self {
            description: value.description,
            name: value.name.map(|x| x.into()),
            interfaces: value.interfaces,
            directives: value.directives,
            fields: value.fields,
        }
    }
}

// TODO this is bad. We should instead convert both GraphQL types to a common
// Isograph type
impl From<GraphQLInputObjectTypeDefinition> for IsographObjectTypeDefinition {
    fn from(value: GraphQLInputObjectTypeDefinition) -> Self {
        Self {
            description: value.description,
            name: value.name.map(|x| x.into()),
            interfaces: vec![],
            directives: value.directives,
            fields: value
                .fields
                .into_iter()
                .map(|with_location| with_location.map(From::from))
                .collect(),
        }
    }
}

/// An object type in the schema.
#[derive(Debug)]
pub struct SchemaObject<TOutputFormat: OutputFormat> {
    pub description: Option<DescriptionValue>,
    pub name: IsographObjectTypeName,
    pub id: ServerObjectId,
    // We probably don't want this
    pub directives: Vec<GraphQLDirective<GraphQLConstantValue>>,
    /// TODO remove id_field from fields, and change the type of Option<ServerFieldId>
    /// to something else.
    pub id_field: Option<ServerStrongIdFieldId>,
    pub encountered_fields:
        BTreeMap<SelectableFieldName, DefinitionLocation<ServerFieldId, SelectionTypeId>>,
    /// Some if the object is concrete; None otherwise.
    pub concrete_type: Option<IsographObjectTypeName>,

    pub output_associated_data: TOutputFormat::SchemaObjectAssociatedData,
}

#[derive(Debug, Clone)]
pub struct SchemaServerField<
    TData,
    TClientFieldVariableDefinitionAssociatedData: Ord + Debug,
    TOutputFormat: OutputFormat,
> {
    pub description: Option<DescriptionValue>,
    /// The name of the server field and the location where it was defined
    /// (an iso literal or Location::Generated).
    pub name: WithLocation<SelectableFieldName>,
    pub id: ServerFieldId,
    pub associated_data: TData,
    pub parent_type_id: ServerObjectId,
    // pub directives: Vec<Directive<ConstantValue>>,
    pub arguments:
        Vec<WithLocation<VariableDefinition<TClientFieldVariableDefinitionAssociatedData>>>,
    // TODO remove this. This is indicative of poor modeling.
    pub is_discriminator: bool,
    pub phantom_data: PhantomData<TOutputFormat>,
}

impl<
        TData,
        TClientFieldVariableDefinitionAssociatedData: Clone + Ord + Debug,
        TOutputFormat: OutputFormat,
    > SchemaServerField<TData, TClientFieldVariableDefinitionAssociatedData, TOutputFormat>
{
    pub fn and_then<TData2, E>(
        &self,
        convert: impl FnOnce(&TData) -> Result<TData2, E>,
    ) -> Result<
        SchemaServerField<TData2, TClientFieldVariableDefinitionAssociatedData, TOutputFormat>,
        E,
    > {
        Ok(SchemaServerField {
            description: self.description,
            name: self.name,
            id: self.id,
            associated_data: convert(&self.associated_data)?,
            parent_type_id: self.parent_type_id,
            arguments: self.arguments.clone(),
            is_discriminator: self.is_discriminator,
            phantom_data: PhantomData,
        })
    }

    pub fn map<TData2, E>(
        &self,
        convert: impl FnOnce(&TData) -> TData2,
    ) -> SchemaServerField<TData2, TClientFieldVariableDefinitionAssociatedData, TOutputFormat>
    {
        SchemaServerField {
            description: self.description,
            name: self.name,
            id: self.id,
            associated_data: convert(&self.associated_data),
            parent_type_id: self.parent_type_id,
            arguments: self.arguments.clone(),
            is_discriminator: self.is_discriminator,
            phantom_data: PhantomData,
        }
    }
}

// TODO make SchemaServerField generic over TData, TId and TArguments, instead of just TData.
// Then, SchemaIdField can be the same struct.
#[derive(Debug, Clone, Copy)]
pub struct SchemaIdField<TData> {
    pub description: Option<DescriptionValue>,
    pub name: WithLocation<SelectableFieldName>,
    pub id: ServerStrongIdFieldId,
    pub associated_data: TData,
    pub parent_type_id: ServerObjectId,
    // pub directives: Vec<Directive<ConstantValue>>,
}

impl<
        TData: Copy,
        TClientFieldVariableDefinitionAssociatedData: Ord + Debug,
        TOutputFormat: OutputFormat,
    > TryFrom<SchemaServerField<TData, TClientFieldVariableDefinitionAssociatedData, TOutputFormat>>
    for SchemaIdField<TData>
{
    type Error = ();

    fn try_from(
        value: SchemaServerField<
            TData,
            TClientFieldVariableDefinitionAssociatedData,
            TOutputFormat,
        >,
    ) -> Result<Self, Self::Error> {
        // If the field is valid as an id field, we succeed, otherwise, fail.
        // Initially, that will mean checking that there are no arguments.
        // This will result in a lot of false positives, and that can be improved
        // by requiring a specific directive or something.
        //
        // There are no arguments now, so this will always succeed.
        //
        // This comment is outdated:
        // Also, before this is called, we have already converted the associated_data to be valid
        // (it should go from TypeAnnotation<T> to NamedTypeAnnotation<T>) via
        // inner_non_null_named_type. We should eventually add some NewType wrapper to
        // enforce that we didn't just call .inner()
        Ok(SchemaIdField {
            description: value.description,
            name: value.name,
            id: value.id.0.into(),
            associated_data: value.associated_data,
            parent_type_id: value.parent_type_id,
        })
    }
}

#[derive(Debug)]
pub struct ClientPointer<
    TSelectionTypeSelectionScalarFieldAssociatedData,
    TSelectionTypeSelectionLinkedFieldAssociatedData,
    TClientFieldVariableDefinitionAssociatedData: Ord + Debug,
    TOutputFormat: OutputFormat,
> {
    pub description: Option<DescriptionValue>,
    pub name: ClientPointerFieldName,
    pub id: ClientPointerId,
    pub to: TypeAnnotation<ServerObjectId>,

    pub reader_selection_set: Vec<
        WithSpan<
            ServerFieldSelection<
                TSelectionTypeSelectionScalarFieldAssociatedData,
                TSelectionTypeSelectionLinkedFieldAssociatedData,
            >,
        >,
    >,

    pub refetch_strategy: RefetchStrategy<
        TSelectionTypeSelectionScalarFieldAssociatedData,
        TSelectionTypeSelectionLinkedFieldAssociatedData,
    >,

    pub variable_definitions:
        Vec<WithSpan<VariableDefinition<TClientFieldVariableDefinitionAssociatedData>>>,

    // Why is this not calculated when needed?
    pub type_and_field: ObjectTypeAndFieldName,

    pub parent_object_id: ServerObjectId,

    pub output_format: PhantomData<TOutputFormat>,
}

#[derive(Debug)]
pub struct ClientField<
    TSelectionTypeSelectionScalarFieldAssociatedData,
    TSelectionTypeSelectionLinkedFieldAssociatedData,
    TClientFieldVariableDefinitionAssociatedData: Ord + Debug,
    TOutputFormat: OutputFormat,
> {
    pub description: Option<DescriptionValue>,
    // TODO make this a ClientFieldName that can be converted into a SelectableFieldName
    pub name: SelectableFieldName,
    pub id: ClientFieldId,
    // TODO model this so that reader_selection_sets are required for
    // non-imperative client fields. (Are imperatively loaded fields
    // true client fields? Probably not!)
    pub reader_selection_set: Option<
        Vec<
            WithSpan<
                ServerFieldSelection<
                    TSelectionTypeSelectionScalarFieldAssociatedData,
                    TSelectionTypeSelectionLinkedFieldAssociatedData,
                >,
            >,
        >,
    >,

    // None -> not refetchable
    pub refetch_strategy: Option<
        RefetchStrategy<
            TSelectionTypeSelectionScalarFieldAssociatedData,
            TSelectionTypeSelectionLinkedFieldAssociatedData,
        >,
    >,

    // TODO we should probably model this differently
    pub variant: ClientFieldVariant,

    pub variable_definitions:
        Vec<WithSpan<VariableDefinition<TClientFieldVariableDefinitionAssociatedData>>>,

    // Why is this not calculated when needed?
    pub type_and_field: ObjectTypeAndFieldName,

    pub parent_object_id: ServerObjectId,
    pub output_format: PhantomData<TOutputFormat>,
}

impl<
        TSelectionTypeSelectionScalarFieldAssociatedData,
        TSelectionTypeSelectionLinkedFieldAssociatedData,
        TClientFieldVariableDefinitionAssociatedData: Ord + Debug,
        TOutputFormat: OutputFormat,
    >
    ClientField<
        TSelectionTypeSelectionScalarFieldAssociatedData,
        TSelectionTypeSelectionLinkedFieldAssociatedData,
        TClientFieldVariableDefinitionAssociatedData,
        TOutputFormat,
    >
{
    pub fn selection_set_for_parent_query(
        &self,
    ) -> &Vec<
        WithSpan<
            ServerFieldSelection<
                TSelectionTypeSelectionScalarFieldAssociatedData,
                TSelectionTypeSelectionLinkedFieldAssociatedData,
            >,
        >,
    > {
        if let Some(s) = self.reader_selection_set.as_ref() {
            s
        } else {
            self.refetch_strategy
                .as_ref()
                .map(|strategy| strategy.refetch_selection_set())
                .expect(
                    "Expected client field to have \
                    either a reader_selection_set or a refetch_selection_set.\
                    This is indicative of a bug in Isograph.",
                )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathToRefetchField {
    pub linked_fields: Vec<NormalizationKey>,
    pub field_name: SelectableFieldName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameAndArguments {
    pub name: SelectableFieldName,
    pub arguments: Vec<ArgumentKeyAndValue>,
}

impl NameAndArguments {
    pub fn normalization_key(&self) -> NormalizationKey {
        if self.name == "id".intern().into() {
            NormalizationKey::Id
        } else {
            NormalizationKey::ServerField(self.clone())
        }
    }
}

impl<T, VariableDefinitionInnerType: Ord + Debug, TOutputFormat: OutputFormat>
    SchemaServerField<T, VariableDefinitionInnerType, TOutputFormat>
{
    // TODO probably unnecessary, and can be replaced with .map and .transpose
    pub fn split(
        self,
    ) -> (
        SchemaServerField<(), VariableDefinitionInnerType, TOutputFormat>,
        T,
    ) {
        let Self {
            description,
            name,
            id,
            associated_data,
            parent_type_id,
            arguments,
            is_discriminator,
            phantom_data,
        } = self;
        (
            SchemaServerField {
                description,
                name,
                id,
                associated_data: (),
                parent_type_id,
                arguments,
                is_discriminator,
                phantom_data,
            },
            associated_data,
        )
    }
}

/// A scalar type in the schema.
#[derive(Debug)]
pub struct SchemaScalar<TOutputFormat: OutputFormat> {
    pub description: Option<WithSpan<DescriptionValue>>,
    pub name: WithLocation<GraphQLScalarTypeName>,
    pub id: ServerScalarId,
    pub javascript_name: JavascriptName,
    pub output_format: PhantomData<TOutputFormat>,
}
