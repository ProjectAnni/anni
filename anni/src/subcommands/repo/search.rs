use std::path::PathBuf;

use anni_repo::{
    search::{tantivy::collector::TopDocs, RepositorySearchManager},
    RepositoryManager,
};
use clap::Args;
use clap_handler::handler;

#[derive(Args, Debug, Clone)]
pub struct RepoSearchAction {
    #[clap(long)]
    path: PathBuf,

    #[clap(long, default_value = "10")]
    limit: usize,
    #[clap(long, default_value = "0")]
    offset: usize,

    keyword: Option<String>,
}

#[handler(RepoSearchAction)]
fn repo_search_action(me: RepoSearchAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;

    match me.keyword {
        Some(keyword) => {
            // search
            let search_manager = RepositorySearchManager::open(me.path)?;
            let reader = search_manager.index.reader()?;
            let searcher = reader.searcher();

            let query_parser = search_manager.build_query_parser();
            let query = query_parser.parse_query(&keyword)?;
            let top_docs: Vec<_> =
                searcher.search(&query, &TopDocs::with_limit(me.limit).and_offset(me.offset))?;

            for (_score, doc_address) in top_docs {
                let document = searcher.doc(doc_address)?;

                let id = search_manager.deserialize_document(document);
                let album = manager.album(&id.album_id).expect("Failed to get album");
                let disc = album
                    .iter()
                    .skip((id.disc_id - 1) as usize)
                    .next()
                    .expect("Failed to get disc");
                let track = disc
                    .iter()
                    .skip((id.track_id - 1) as usize)
                    .next()
                    .expect("Failed to get track");
                println!("title = {}, artist = {}", track.title(), track.artist());
            }
        }
        None => {
            // build index
            manager.build_search_index(me.path);
        }
    }
    Ok(())
}
