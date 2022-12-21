use std::path::PathBuf;

use anni_repo::{
    search::{
        open_indexer,
        tantivy::{collector::TopDocs, query::QueryParser, DocAddress, Score},
        SearchFields,
    },
    RepositoryManager,
};
use clap::Args;
use clap_handler::handler;

#[derive(Args, Debug, Clone)]
pub struct RepoSearchAction {
    #[clap(long)]
    path: PathBuf,

    keyword: Option<String>,
}

#[handler(RepoSearchAction)]
fn repo_search_action(me: RepoSearchAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;

    match me.keyword {
        Some(keyword) => {
            // search
            let (index, SearchFields { title, artist, .. }) = open_indexer(me.path);

            let reader = index.reader().unwrap();
            let searcher = reader.searcher();
            let schema = index.schema();

            let query_parser = QueryParser::for_index(&index, vec![title, artist]);
            let query = query_parser.parse_query(&keyword).unwrap();
            let top_docs: Vec<(Score, DocAddress)> =
                searcher.search(&query, &TopDocs::with_limit(10)).unwrap();

            for (_score, doc_address) in top_docs {
                let retrieved_doc = searcher.doc(doc_address).unwrap();
                println!("{}", schema.to_json(&retrieved_doc));
            }
        }
        None => {
            // build index
            manager.build_search_index(me.path);
        }
    }
    Ok(())
}
