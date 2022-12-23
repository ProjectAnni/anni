use std::path::Path;

use lindera_tantivy::{
    mode::Mode,
    tokenizer::{DictionaryConfig, DictionaryKind, LinderaTokenizer, TokenizerConfig},
};
use tantivy::{
    doc,
    query::QueryParser,
    schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED},
    Document, Index, TantivyError,
};

pub use tantivy;
use uuid::Uuid;

use crate::prelude::TrackIdentifier;

pub struct RepositorySearchManager {
    pub index: Index,
    fields: SearchFields,
}

impl RepositorySearchManager {
    pub fn create<P>(directory_path: P) -> Result<Self, TantivyError>
    where
        P: AsRef<Path>,
    {
        let (schema, fields) = SearchFields::new();
        let index = Index::create_in_dir(directory_path, schema)?;
        let me = Self { index, fields };
        me.register_tokenizers();
        Ok(me)
    }

    pub fn open<P>(directory_path: P) -> Result<Self, TantivyError>
    where
        P: AsRef<Path>,
    {
        let index = Index::open_in_dir(directory_path)?;
        let fields = SearchFields::from_index(&index);
        let me = Self { index, fields };
        me.register_tokenizers();
        Ok(me)
    }

    fn register_tokenizers(&self) {
        let dictionary = DictionaryConfig {
            kind: Some(DictionaryKind::IPADIC),
            path: None,
        };

        let config = TokenizerConfig {
            dictionary,
            user_dictionary: None,
            mode: Mode::Normal,
        };

        self.index
            .tokenizers()
            .register("lang_ja", LinderaTokenizer::with_config(config).unwrap());
    }

    pub fn build_document(
        &self,
        title: &str,
        artist: &str,
        album_id: &Uuid,
        disc_id: i64,
        track_id: i64,
    ) -> Document {
        doc!(
            self.fields.title => title,
            self.fields.artist => artist,
            self.fields.album_id => &album_id.as_bytes()[..],
            self.fields.disc_id => disc_id,
            self.fields.track_id => track_id,
        )
    }

    pub fn build_query_parser(&self) -> QueryParser {
        QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.artist])
    }

    pub fn deserialize_document(&self, doc: Document) -> TrackIdentifier {
        let album_id = doc.get_first(self.fields.album_id).unwrap();
        let disc_id = doc.get_first(self.fields.disc_id).unwrap();
        let track_id = doc.get_first(self.fields.track_id).unwrap();

        TrackIdentifier {
            album_id: Uuid::from_slice(album_id.as_bytes().unwrap()).unwrap(),
            disc_id: disc_id.as_i64().unwrap() as u32,
            track_id: track_id.as_i64().unwrap() as u32,
        }
    }
}

struct SearchFields {
    pub title: Field,
    pub artist: Field,
    pub album_id: Field,
    pub disc_id: Field,
    pub track_id: Field,
}

impl SearchFields {
    pub fn new() -> (Schema, Self) {
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
        let album_id = schema_builder.add_bytes_field("album_id", STORED);
        let disc_id = schema_builder.add_i64_field("disc_id", STORED);
        let track_id = schema_builder.add_i64_field("track_id", STORED);

        (
            schema_builder.build(),
            Self {
                title,
                artist,
                album_id,
                disc_id,
                track_id,
            },
        )
    }

    fn from_index(index: &Index) -> Self {
        let schema = index.schema();
        let title = schema.get_field("title").unwrap();
        let artist = schema.get_field("artist").unwrap();
        let album_id = schema.get_field("album_id").unwrap();
        let disc_id = schema.get_field("disc_id").unwrap();
        let track_id = schema.get_field("track_id").unwrap();

        Self {
            title,
            artist,
            album_id,
            disc_id,
            track_id,
        }
    }
}
