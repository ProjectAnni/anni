use crate::RepoSubcommand;
use clap::Args;
use clap_handler::handler;
use notify::event::{AccessKind, AccessMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Receiver};

#[derive(Args, Debug, Clone)]
pub struct RepoWatchAction;

#[handler(RepoWatchAction)]
fn repo_watch(_: RepoWatchAction, repo: RepoSubcommand) -> anyhow::Result<()> {
    let root = repo.root;
    async_watch(root).await?;
    Ok(())
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

async fn async_watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
    let (mut watcher, mut rx) = async_watcher()?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    while let Some(res) = rx.recv().await {
        match res {
            Ok(event) => match event.kind {
                EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                    log::info!("modified: {:?}", event.paths);
                }
                EventKind::Create(_) => {
                    log::info!("created: {:?}", event.paths);
                }
                EventKind::Remove(notify::event::RemoveKind::Any) => {
                    log::info!("removed: {:?}", event.paths);
                }
                _ => {}
            },
            Err(e) => log::error!("watch error: {:?}", e),
        }
    }

    Ok(())
}
