use crate::{ClientField, ClientFieldVariant, OutputFormat, UnvalidatedSchema, LINK_FIELD_NAME};
use common_lang_types::{Location, ObjectTypeAndFieldName, WithLocation};
use intern::string_key::Intern;
use isograph_lang_types::{DefinitionLocation, SelectionType};

use super::create_additional_fields_error::{
    CreateAdditionalFieldsError, ProcessTypeDefinitionResult,
};

impl<TOutputFormat: OutputFormat> UnvalidatedSchema<TOutputFormat> {
    pub fn add_link_fields(&mut self) -> ProcessTypeDefinitionResult<()> {
        for object in &mut self.server_field_data.server_objects {
            let field_name = *LINK_FIELD_NAME;
            let next_client_field_id = self.client_types.len().into();
            self.client_types.push(SelectionType::Scalar(ClientField {
                description: Some(
                    format!("A store Link for the {} type.", object.name)
                        .intern()
                        .into(),
                ),
                id: next_client_field_id,
                name: field_name,
                parent_object_id: object.id,
                variable_definitions: vec![],
                reader_selection_set: vec![],
                variant: ClientFieldVariant::Link,
                type_and_field: ObjectTypeAndFieldName {
                    field_name: field_name.into(),
                    type_name: object.name,
                },
                refetch_strategy: None,
                output_format: std::marker::PhantomData,
            }));

            if object
                .encountered_fields
                .insert(
                    field_name.into(),
                    DefinitionLocation::Client(SelectionType::Scalar(next_client_field_id)),
                )
                .is_some()
            {
                return Err(WithLocation::new(
                    CreateAdditionalFieldsError::FieldExistsOnType {
                        field_name: field_name.into(),
                        parent_type: object.name,
                    },
                    Location::generated(),
                ));
            }
        }
        Ok(())
    }
}
