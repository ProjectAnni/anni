use anni_workspace::{AnniWorkspace, WorkspaceAlbumState};
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use std::env::current_dir;
use std::fmt::{Display, Formatter};
use std::path::Path;
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceStatusAction {
    #[clap(short = 'a', long)]
    album_id: bool,
    #[clap(short = 'j', long)]
    json: bool,
}

struct DisplayUuid<'uuid> {
    inner: &'uuid Uuid,
    full: bool,
}

impl<'uuid> DisplayUuid<'uuid> {
    fn new(inner: &'uuid Uuid, full: bool) -> Self {
        Self { inner, full }
    }
}

impl Display for DisplayUuid<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.full {
            write!(f, "{}", self.inner)
        } else {
            let to_display = &self.inner.to_string()[0..8];
            write!(f, "{to_display}")
        }
    }
}

#[handler(WorkspaceStatusAction)]
pub async fn handle_workspace_status(me: WorkspaceStatusAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let albums = workspace.scan()?;

    let root = workspace.workspace_root();

    if me.json {
        let json = serde_json::to_string(&albums)?;
        println!("{json}");
    } else {
        let mut untracked: Vec<(&Path, DisplayUuid)> = vec![];
        let mut committed: Vec<(&Path, DisplayUuid)> = vec![];
        let mut dangling: Vec<(&Path, DisplayUuid)> = vec![];
        let mut garbage: Vec<DisplayUuid> = vec![];
        for album in albums.iter() {
            match album.state {
                WorkspaceAlbumState::Untracked(ref p) => untracked.push((
                    p.strip_prefix(&root)?,
                    DisplayUuid::new(&album.album_id, me.album_id),
                )),
                WorkspaceAlbumState::Committed(ref p) => committed.push((
                    p.strip_prefix(&root)?,
                    DisplayUuid::new(&album.album_id, me.album_id),
                )),
                WorkspaceAlbumState::Dangling(ref p) => dangling.push((
                    p.strip_prefix(&root)?,
                    DisplayUuid::new(&album.album_id, me.album_id),
                )),
                WorkspaceAlbumState::Garbage => {
                    garbage.push(DisplayUuid::new(&album.album_id, me.album_id))
                }
            }
        }

        if !untracked.is_empty() {
            println!("Untracked albums:");
            for (path, album_id) in untracked {
                let album_id = format!("[{album_id}]").bold();
                let output = format!("{album_id}: {}", path.display()).bright_red();
                println!("\t{output}");
            }
            println!();
        }

        if !committed.is_empty() {
            println!("Committed albums:");
            for (path, album_id) in committed {
                let album_id = format!("[{album_id}]").bold();
                let output = format!("{album_id}: {}", path.display()).green();
                println!("\t{output}");
            }
            println!();
        }

        if !dangling.is_empty() {
            println!("Dangling albums:");
            for (path, album_id) in dangling {
                let album_id = format!("[{album_id}]").bold();
                let output = format!("{album_id}: {}", path.display()).red();
                println!("\t{output}");
            }
            println!();
        }

        if !garbage.is_empty() {
            println!("Garbage albums:");
            for album_id in garbage {
                let output = format!("{}", album_id).white();
                println!("\t{output}");
            }
            println!();
        }
    }

    Ok(())
}
