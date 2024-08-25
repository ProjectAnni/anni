use crate::{
    query::tag::{Tag, TagType},
    schema,
};

#[derive(cynic::QueryVariables, Debug)]
pub struct AddTagVariables<'a> {
    pub name: &'a str,
    #[cynic(rename = "type")]
    pub type_: TagType,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataMutation", variables = "AddTagVariables")]
pub struct AddTagMutation {
    #[arguments(name: $name, type: $type_)]
    pub add_tag: Tag,
}
