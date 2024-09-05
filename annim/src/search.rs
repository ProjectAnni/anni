use std::{path::Path, sync::Arc};

use lindera_core::mode::Mode;
use lindera_dictionary::{DictionaryConfig, DictionaryKind, DictionaryLoader};
use lindera_tantivy::tokenizer::LinderaTokenizer;
use tantivy::{
    directory::MmapDirectory,
    doc,
    query::{Query, QueryParser},
    schema::{
        Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, INDEXED, STORED,
    },
    Index, IndexReader, IndexWriter, Opstamp, Searcher, TantivyDocument, TantivyError,
};
use tokio::sync::{RwLock, RwLockReadGuard};

pub struct RepositorySearchManager {
    index: Index,
    index_reader: IndexReader,
    index_writer: Arc<RwLock<IndexWriter>>,
    query_parser: QueryParser,

    pub fields: SearchFields,
}

impl RepositorySearchManager {
    pub fn open_or_create<P>(directory_path: P) -> Result<Self, TantivyError>
    where
        P: AsRef<Path>,
    {
        let (schema, fields) = SearchFields::new();
        let directory = MmapDirectory::open(directory_path)?;
        let index = Index::open_or_create(directory, schema)?;

        let reader = index.reader()?;
        let writer = index.writer(50_000_000)?;
        let query_parser = QueryParser::for_index(&index, vec![fields.title, fields.artist]);

        let me = Self {
            index,
            index_reader: reader,
            index_writer: Arc::new(RwLock::new(writer)),
            query_parser,
            fields,
        };
        me.register_tokenizers();
        Ok(me)
    }

    pub async fn writer(&self) -> SearchWriter<'_> {
        SearchWriter {
            lock: self.index_writer.read().await,
            manager: self,
        }
    }

    async fn commit(&self) -> Result<(), TantivyError> {
        let mut writer = self.index_writer.write().await;
        writer.commit()?;
        writer.garbage_collect_files().await?;

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

    pub fn searcher(&self) -> Searcher {
        self.index_reader.searcher()
    }

    pub fn query_parser(&self) -> &QueryParser {
        &self.query_parser
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

pub struct SearchWriter<'a> {
    lock: RwLockReadGuard<'a, IndexWriter>,
    manager: &'a RepositorySearchManager,
}

impl SearchWriter<'_> {
    pub fn add_document(&self, document: TantivyDocument) -> tantivy::Result<Opstamp> {
        self.lock.add_document(document)
    }

    pub fn delete_query(&self, query: Box<dyn Query>) -> tantivy::Result<Opstamp> {
        self.lock.delete_query(query)
    }

    pub async fn commit(self) -> tantivy::Result<()> {
        let manager = self.manager;
        drop(self);
        manager.commit().await
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
}
