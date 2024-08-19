use crate::entities::*;
use async_graphql::dynamic::*;
use sea_orm::DatabaseConnection;
use seaography::{Builder, BuilderContext};

lazy_static::lazy_static! { static ref CONTEXT : BuilderContext = BuilderContext :: default () ; }

pub fn schema(
    database: DatabaseConnection,
    depth: Option<usize>,
    complexity: Option<usize>,
) -> Result<Schema, SchemaError> {
    let mut builder = Builder::new(&CONTEXT, database.clone());

    // builder.register_entity::<album::Entity>(
    //     <album::RelatedEntity as sea_orm::Iterable>::iter()
    //         .map(|rel| seaography::RelationBuilder::get_relation(&rel, builder.context))
    //         .collect(),
    // );
    // builder = builder.register_entity_dataloader_one_to_one(album::Entity, tokio::spawn);
    // builder = builder.register_entity_dataloader_one_to_many(album::Entity, tokio::spawn);
    builder.register_entity_mutations::<album::Entity, album::ActiveModel>();

    // builder.register_entity::<disc::Entity>(
    //     <disc::RelatedEntity as sea_orm::Iterable>::iter()
    //         .map(|rel| seaography::RelationBuilder::get_relation(&rel, builder.context))
    //         .collect(),
    // );
    // builder = builder.register_entity_dataloader_one_to_one(disc::Entity, tokio::spawn);
    // builder = builder.register_entity_dataloader_one_to_many(disc::Entity, tokio::spawn);
    builder.register_entity_mutations::<disc::Entity, disc::ActiveModel>();

    // builder.register_entity::<track::Entity>(
    //     <track::RelatedEntity as sea_orm::Iterable>::iter()
    //         .map(|rel| seaography::RelationBuilder::get_relation(&rel, builder.context))
    //         .collect(),
    // );
    // builder = builder.register_entity_dataloader_one_to_one(track::Entity, tokio::spawn);
    // builder = builder.register_entity_dataloader_one_to_many(track::Entity, tokio::spawn);
    builder.register_entity_mutations::<track::Entity, track::ActiveModel>();

    let schema = builder.schema_builder();
    let schema = if let Some(depth) = depth {
        schema.limit_depth(depth)
    } else {
        schema
    };
    let schema = if let Some(complexity) = complexity {
        schema.limit_complexity(complexity)
    } else {
        schema
    };
    schema.data(database).finish()
}
