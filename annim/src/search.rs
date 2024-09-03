use std::{
    path::Path,
    sync::{Arc, LazyLock},
};

use lindera_core::mode::Mode;
use lindera_dictionary::{DictionaryConfig, DictionaryKind, DictionaryLoader};
use lindera_tantivy::tokenizer::LinderaTokenizer;
use tantivy::{
    directory::{MmapDirectory, RamDirectory},
    doc,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, INDEXED, STORED,
    },
    Index, IndexWriter, TantivyDocument, TantivyError,
};
use tokio::sync::{RwLock, RwLockReadGuard};

pub struct RepositorySearchManager {
    pub index: Index,
    index_writer: Arc<RwLock<IndexWriter>>,

    pub fields: SearchFields,
}

impl RepositorySearchManager {
    pub fn open_or_create<P>(directory_path: P) -> Result<Self, TantivyError>
    where
        P: AsRef<Path>,
    {
        let (schema, fields) = SearchFields::new();
        let directory = MmapDirectory::open(directory_path)?;
        let mut index = Index::open_or_create(directory, schema)?;
        let writer = index.writer(50_000_000)?;

        let me = Self {
            index,
            index_writer: Arc::new(RwLock::new(writer)),
            fields,
        };
        me.register_tokenizers();
        Ok(me)
    }

    pub async fn writer_read(&self) -> RwLockReadGuard<'_, IndexWriter> {
        self.index_writer.read().await
    }

    pub async fn commit(&self) -> Result<(), TantivyError> {
        self.index_writer.write().await.commit()?;

        Ok(())
    }

    fn register_tokenizers(&self) {
        let dictionary_config = DictionaryConfig {
            kind: Some(DictionaryKind::IPADIC),
            path: None,
        };
        let dictionary = DictionaryLoader::load_dictionary_from_config(dictionary_config).unwrap();

        self.index.tokenizers().register(
            "lang_ja",
            LinderaTokenizer::new(dictionary, None, Mode::Normal),
        );
    }

    pub fn build_track_document(
        &self,
        title: &str,
        artist: &str,
        album_db_id: i64,
        disc_db_id: Option<i64>,
        track_db_id: Option<i64>,
    ) -> TantivyDocument {
        doc!(
            self.fields.album_db_id => album_db_id,
            self.fields.disc_db_id => disc_db_id.unwrap_or(i64::MAX),
            self.fields.track_db_id => track_db_id.unwrap_or(i64::MAX),
            self.fields.title => title,
            self.fields.artist => artist,
        )
    }

    pub fn build_query_parser(&self) -> QueryParser {
        QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.artist])
    }

    pub fn deserialize_document(&self, doc: TantivyDocument) -> (i64, Option<i64>, Option<i64>) {
        let album_db_id = doc.get_first(self.fields.album_db_id).unwrap();
        let disc_db_id = doc.get_first(self.fields.disc_db_id).unwrap();
        let track_db_id = doc.get_first(self.fields.track_db_id).unwrap();

        (
            album_db_id.as_i64().unwrap(),
            disc_db_id
                .as_i64()
                .and_then(|i| if i == i64::MAX { None } else { Some(i) }),
            track_db_id
                .as_i64()
                .and_then(|i| if i == i64::MAX { None } else { Some(i) }),
        )
    }
}

pub struct SearchFields {
    pub album_db_id: Field,
    pub disc_db_id: Field,
    pub track_db_id: Field,

    pub title: Field,
    pub artist: Field,
}

impl SearchFields {
    pub fn new() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();
        let album_db_id = schema_builder.add_i64_field("album_db_id", STORED | INDEXED);
        let disc_db_id = schema_builder.add_i64_field("disc_db_id", STORED | INDEXED);
        let track_db_id = schema_builder.add_i64_field("track_db_id", STORED | INDEXED);

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

        (
            schema_builder.build(),
            Self {
                album_db_id,
                disc_db_id,
                track_db_id,
                title,
                artist,
            },
        )
    }

    fn from_index(index: &Index) -> Self {
        let schema = index.schema();
        let album_db_id = schema.get_field("album_db_id").unwrap();
        let disc_db_id = schema.get_field("disc_db_id").unwrap();
        let track_db_id = schema.get_field("track_db_id").unwrap();
        let title = schema.get_field("title").unwrap();
        let artist = schema.get_field("artist").unwrap();

        Self {
            album_db_id,
            disc_db_id,
            track_db_id,
            title,
            artist,
        }
    }
}
