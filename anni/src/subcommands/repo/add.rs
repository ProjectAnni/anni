use crate::repo::is_album_folder;
use crate::{ball, ll};
use anni_common::fs;
use anni_flac::error::FlacError;
use anni_flac::FlacHeader;
use anni_repo::library::{file_name, AlbumFolderInfo, DiscFolderInfo};
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Args, Debug, Clone)]
pub struct RepoAddAction {
    #[clap(short = 'e', long)]
    #[clap(help = ll!("repo-add-edit"))]
    open_editor: bool,

    #[clap(short = 'D', long = "duplicate")]
    allow_duplicate: bool,

    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(RepoAddAction)]
fn repo_add(me: RepoAddAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    for to_add in me.directories.into_iter() {
        let last = file_name(&to_add)?;
        if !is_album_folder(&last) {
            ball!("repo-invalid-album", name = last);
        }

        let AlbumFolderInfo {
            release_date,
            catalog,
            title: album_title,
            edition,
            disc_count,
        } = AlbumFolderInfo::from_str(&last)?;

        let mut directories = fs::get_subdirectories(&to_add)?;
        if disc_count == 1 {
            directories.push(to_add);
        }
        if disc_count != directories.len() {
            bail!("Subdirectory count != disc number!")
        }

        let discs = directories
            .iter()
            .map(|dir| {
                let mut files = fs::get_ext_files(PathBuf::from(dir), "flac", false)?;
                if files.is_empty() {
                    bail!("No FLAC files found in {}", dir.display())
                }

                alphanumeric_sort::sort_path_slice(&mut files);
                let disc = if disc_count > 1 {
                    let DiscFolderInfo { info, .. } = DiscFolderInfo::from_str(&*file_name(dir)?)?;
                    info
                } else {
                    DiscInfo::new(catalog.clone(), None, None, None, Default::default())
                };
                let tracks = files
                    .iter()
                    .map(|path| {
                        let header = FlacHeader::from_file(path)?;
                        Ok(header.into())
                    })
                    .collect::<Result<Vec<_>, FlacError>>()?;

                Ok(Disc::new(disc, tracks))
            })
            .collect::<Result<_, _>>()?;

        let album = Album::new(
            AlbumInfo {
                title: album_title,
                edition,
                release_date,
                catalog: catalog.clone(),
                ..Default::default()
            },
            discs,
        );

        manager.add_album(&catalog, &album, me.allow_duplicate)?;
        if me.open_editor {
            for file in manager.album_paths(&catalog)? {
                edit::edit_file(&file)?;
            }
        }
    }
    Ok(())
}
