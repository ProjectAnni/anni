use crate::{ball, ll};
use anni_common::fs;
use anni_provider::fs::LocalFileSystemProvider;
use anni_provider::providers::CommonConventionProvider;
use anni_provider::strict_album_path;
use anni_repo::db::RepoDatabaseRead;
use anni_repo::library::AlbumFolderInfo;
use anni_repo::RepositoryManager;
use clap::{Args, Subcommand};
use clap_handler::{handler, Context, Handler};
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll!("library"))]
#[handler_inject(library_fields)]
pub struct LibrarySubcommand {
    #[clap(long = "repo", env = "ANNI_REPO")]
    repo_root: PathBuf,

    #[clap(subcommand)]
    action: LibraryAction,
}

impl LibrarySubcommand {
    async fn library_fields(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let manager = RepositoryManager::new(self.repo_root.as_path())?;
        ctx.insert(manager);
        Ok(())
    }
}

#[derive(Subcommand, Debug, Clone, Handler)]
pub enum LibraryAction {
    #[clap(name = "tag", alias = "apply")]
    #[clap(about = ll!("library-tag"))]
    ApplyTag(LibraryApplyTagAction),
    Link(LibraryLinkAction),
}

#[derive(Args, Debug, Clone)]
pub struct LibraryApplyTagAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(LibraryApplyTagAction)]
pub fn library_apply_tag(
    me: LibraryApplyTagAction,
    manager: RepositoryManager,
) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;
    for path in me.directories {
        if !path.is_dir() {
            anyhow::bail!("{} is not a directory", path.display());
        }

        let path = path.canonicalize()?;
        let folder_name = path
            .file_name()
            .expect("Failed to get basename of path")
            .to_string_lossy();
        if is_uuid(&folder_name) {
            // strict folder structure, folder name is album_id
            let album = manager
                .albums()
                .get(&Uuid::parse_str(folder_name.as_ref())?)
                .ok_or_else(|| anyhow::anyhow!("Album {} not found", folder_name))?;
            album.apply_strict(&path)?;
        } else if let Ok(AlbumFolderInfo {
            release_date,
            catalog,
            title: album_title,
            edition,
            disc_count,
        }) = AlbumFolderInfo::from_str(&folder_name)
        {
            debug!(target: "repo|apply", "Release date: {}, Catalog: {}, Title: {}", release_date, catalog, album_title);

            // convention folder structure, load album by catalog
            let albums = manager.repo.load_albums(&catalog)?;
            let albums = if albums.len() > 1 {
                albums
                    .into_iter()
                    .filter(|a| a.title_raw() == album_title && a.edition() == edition.as_deref())
                    .collect()
            } else {
                albums
            };
            if albums.is_empty() {
                // no album found
                ball!("repo-album-not-found", catalog = catalog);
            }

            // get track metadata & compare with album folder
            let album = albums.into_iter().nth(0).unwrap();
            if album.title_raw() != album_title
                || album.edition() != edition.as_deref()
                || album.catalog() != catalog
                || album.release_date() != &release_date
            {
                ball!("repo-album-info-mismatch");
            }

            // check discs & tracks
            if album.discs_len() != disc_count {
                bail!("discs.len() != disc_count!");
            }
            album.apply_convention(&path)?;
        } else {
            anyhow::bail!("{} is not a valid album id", folder_name);
        }
    }
    Ok(())
}

fn is_uuid(input: &str) -> bool {
    regex::Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .unwrap()
        .is_match(input)
}

#[derive(Args, Debug, Clone)]
pub struct LibraryLinkAction {
    #[clap(short, long, default_value = "2")]
    layer: usize,

    #[clap(long)]
    incremental: bool,

    from: PathBuf,
    to: PathBuf,
}

#[handler(LibraryLinkAction)]
pub async fn library_link(me: LibraryLinkAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;
    let from = me.from.canonicalize()?;
    let to = me.to;
    if !from.is_dir() {
        anyhow::bail!("Must migrate from a directory!");
    }

    if !me.incremental {
        // 1. recreate `to` folder
        fs::remove_dir_all(&to, true)?; // this function only remove sym link and does not remove the underlying file
        fs::create_dir_all(&to)?;
    }

    // 2. create temp database
    let repo_path = to.join("repo.db");
    manager.to_database(&repo_path)?;

    // 3. scan `from` folder
    let provider = CommonConventionProvider::new(
        from,
        RepoDatabaseRead::new(&repo_path.to_string_lossy().to_string())?,
        Box::new(LocalFileSystemProvider),
    )
    .await?;
    for (album_id, album_from) in provider.albums {
        // 4. create album_id folder
        let album_to = strict_album_path(&to, &album_id, me.layer);
        if me.incremental && album_to.exists() {
            continue;
        }
        fs::create_dir_all(&album_to)?;

        // 5. link album art
        fs::symlink_file(
            album_from.path.join("cover.jpg"),
            album_to.join("cover.jpg"),
        )?;

        let discs = vec![album_from];
        let discs = provider.discs.get(&album_id).unwrap_or(&discs);
        for (i, disc_from) in discs.iter().enumerate() {
            // 6. create disc folder
            let disc_to = album_to.join(format!("{}", i + 1));
            fs::create_dir_all(&disc_to)?;

            // 7. link disc art
            fs::symlink_file(disc_from.path.join("cover.jpg"), disc_to.join("cover.jpg"))?;

            // 8. link tracks
            for entry in fs::read_dir(&disc_from.path)? {
                let entry = entry?;
                let parts = entry.file_name();
                let parts = parts.to_string_lossy();
                let parts: Vec<_> = parts.split('.').collect();
                if let Some(&"flac") = parts.last() {
                    let index = parts.first().unwrap();
                    let track_from = entry.path();
                    let track_to = disc_to.join(format!("{}.flac", index.trim_start_matches('0')));
                    fs::symlink_file(track_from, track_to)?;
                }
            }
        }
    }

    Ok(())
}
