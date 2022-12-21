use std::path::Path;

use lindera_tantivy::{
    mode::Mode,
    tokenizer::{DictionaryConfig, DictionaryKind, LinderaTokenizer, TokenizerConfig},
};
use tantivy::{
    schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED, TEXT},
    Index,
};

pub use tantivy;

pub fn open_indexer<P>(dir: P) -> (Index, SearchFields)
where
    P: AsRef<Path>,
{
    let (schema, fields) = prepare_schema();
    let directory = tantivy::directory::MmapDirectory::open(dir).unwrap();
    let index = Index::open_or_create(directory, schema).unwrap();

    let dictionary = DictionaryConfig {
        kind: Some(DictionaryKind::IPADIC),
        path: None,
    };

    let config = TokenizerConfig {
        dictionary,
        user_dictionary: None,
        mode: Mode::Normal,
    };

    index
        .tokenizers()
        .register("lang_ja", LinderaTokenizer::with_config(config).unwrap());

    (index, fields)
}

pub struct SearchFields {
    pub title: Field,
    pub artist: Field,
    pub album_id: Field,
    pub disc_id: Field,
    pub track_id: Field,
}

pub fn prepare_schema() -> (Schema, SearchFields) {
    let mut schema_builder = Schema::builder();
    let title = schema_builder.add_text_field(
        "title",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("lang_ja")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    let artist = schema_builder.add_text_field(
        "artist",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("lang_ja")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    let album_id = schema_builder.add_text_field("album_id", TEXT | STORED);
    let disc_id = schema_builder.add_i64_field("disc_id", STORED);
    let track_id = schema_builder.add_i64_field("track_id", STORED);

    (
        schema_builder.build(),
        SearchFields {
            title,
            artist,
            album_id,
            disc_id,
            track_id,
        },
    )
}
