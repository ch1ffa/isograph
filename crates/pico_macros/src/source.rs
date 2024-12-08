use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

pub(crate) fn source(_args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_name = input.clone().ident;
    let key = struct_name.to_string();

    let fields = match input.data {
        Data::Struct(ref data) => match &data.fields {
            Fields::Named(fields) => fields.named.clone(),
            _ => {
                return syn::Error::new_spanned(&data.fields, "expected named fields")
                    .to_compile_error()
                    .into()
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "expected a struct")
                .to_compile_error()
                .into()
        }
    };

    let field_names = fields.iter().map(|f| &f.ident).collect::<Vec<_>>();
    let field_types = fields.iter().map(|f| &f.ty).collect::<Vec<_>>();

    let output = quote! {
        #[derive(Debug, Clone, PartialEq, Eq)]
        #input

        impl #struct_name {
            pub fn set(
                db: &mut pico::database::Database,
                static_key: &'static str,
                #(#field_names: #field_types),*
            ) {
                db.current_epoch += 1;
                let param_id = pico::params::get_param_id(db, static_key);
                let node_id = pico::node::NodeId::new::<pico::node::SourceNode>(#key, param_id);
                let value = #struct_name {
                    #(#field_names),*
                };
                db.nodes.insert(node_id, pico::node::SourceNode {
                    value: Box::new(value),
                    time_calculated: db.current_epoch,
                });
            }

            pub fn get(db: &mut pico::database::Database, static_key: &'static str) -> Self {
                let param_id = pico::params::get_param_id(db, static_key);
                let node_id = pico::node::NodeId::new::<pico::node::SourceNode>(#key, param_id);
                let node = db.nodes
                    .get::<pico::node::SourceNode>(&node_id)
                    .expect("unexpected node type");
                if let Some(dependencies) = db.dependency_stack.last_mut() {
                    dependencies.push((
                        node.time_calculated,
                        pico::node::Dependency {
                            node_to: node_id,
                            time_verified_or_calculated: db.current_epoch,
                        },
                    ));
                }
                node.value
                    .as_any()
                    .downcast_ref::<Self>()
                    .expect("unexpected struct type")
                    .clone()
            }
        }
    };

    output.into()
}
