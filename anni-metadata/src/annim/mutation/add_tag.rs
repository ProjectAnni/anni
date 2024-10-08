use crate::annim::{
    query::{album::TagTypeInput, tag::Tag},
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct AddTagVariables<'a> {
    pub name: &'a str,
    #[cynic(rename = "type")]
    pub type_: TagTypeInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataMutation", variables = "AddTagVariables")]
pub struct AddTagMutation {
    #[arguments(name: $name, type: $type_)]
    pub add_tag: Tag,
}
