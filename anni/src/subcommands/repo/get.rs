use crate::{ball, ll};
use anni_common::fs;
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use anni_vgmdb::VGMClient;
use chrono::Datelike;
use clap::{Args, Subcommand};
use clap_handler::{handler, Handler};
use cuna::Cuna;
use musicbrainz_rs::entity::artist_credit::ArtistCredit;
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::Fetch;
use std::path::PathBuf;

#[derive(Args, Handler, Debug, Clone)]
pub struct RepoGetAction {
    #[clap(long, global = true)]
    #[clap(help = ll!("repo-get-print"))]
    print: bool,
    #[clap(subcommand)]
    subcommand: RepoGetSubcommand,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum RepoGetSubcommand {
    #[clap(name = "vgmdb")]
    VGMdb(RepoGetVGMdb),
    #[clap(name = "cue")]
    Cue(RepoGetCue),
    #[clap(name = "musicbrainz")]
    Musicbrainz(RepoGetMusicbrainz),
}

async fn search_album(keyword: &str) -> anyhow::Result<Album> {
    let client = VGMClient::default();
    let search = client.search_albums(keyword).await?;
    let album_got = search.into_album(None).await?;

    let release_date = {
        let split = album_got.release_date().split('-').collect::<Vec<_>>();
        AnniDate::from_parts(
            split[0],
            split.get(1).unwrap_or(&"0"),
            split.get(2).unwrap_or(&"0"),
        )
    };

    let discs = album_got
        .discs
        .iter()
        .map(|disc_got| {
            let disc = DiscInfo::new(
                album_got.catalog().unwrap_or("").to_string(),
                Some(disc_got.title.to_string()),
                None,
                None,
                Default::default(),
            );

            let tracks = disc_got
                .tracks
                .iter()
                .map(|track| {
                    let title = track.get().unwrap().to_string();
                    let track_type = TrackType::guess(&title);
                    TrackInfo::new(title, Some("".to_string()), track_type, Default::default())
                })
                .collect();

            Disc::new(disc, tracks)
        })
        .collect();

    Ok(Album::new(
        AlbumInfo {
            title: album_got.title().unwrap().to_string().into(),
            release_date,
            catalog: album_got.catalog().unwrap_or("").to_string(),
            ..Default::default()
        },
        discs,
    ))
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetVGMdb {
    #[clap(short = 'k', long)]
    keyword: Option<String>,

    catalog: String,
}

#[handler(RepoGetVGMdb)]
fn repo_get_vgmdb(
    options: RepoGetVGMdb,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let catalog = &options.catalog;

    let mut album = search_album(&options.keyword.as_deref().unwrap_or(catalog)).await?;

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        album.catalog = options.catalog;
        manager.add_album(album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetCue {
    #[clap(short = 'k', long, help = ll!("repo-get-cue-keyword"))]
    keyword: Option<String>,
    #[clap(short = 'c', long, help = ll!("repo-get-cue-catalog"))]
    catalog: Option<String>,

    path: PathBuf,
}

#[handler(RepoGetCue)]
async fn repo_get_cue(
    options: &RepoGetCue,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let path = &options.path;

    let s = fs::read_to_string(path)?;
    let cue = Cuna::new(&s)?;
    let mut album = match (cue.catalog(), options.keyword.as_ref()) {
        // if catalog is found, fetch metadata from vgmdb
        (Some(catalog), _) => search_album(&catalog.to_string()).await?,
        // otherwise try to search with keyword
        (None, Some(keyword)) => {
            warn!(
                "catalog is unavailable, trying to search vgmdb with keyword `{}`",
                keyword
            );
            search_album(&keyword.to_string()).await?
        }
        // if none is available, try to search with `TITLE` filed in the cue file
        (None, None) => match cue.title().first() {
            Some(title) => {
                warn!("catalog is unavailable, trying to search vgmdb with title `{}`, which may be inaccurate", title);
                search_album(&title.to_string()).await?
            }
            None => ball!("repo-cue-insufficient-information"),
        },
    };

    if album.catalog().is_empty() {
        match &options.catalog {
            Some(catalog) => album.catalog = catalog.to_string(),
            None => ball!("repo-cue-insufficient-information"),
        }
    }

    // set artist if performer exists
    let performer = cue.performer().first();
    if let Some(performer) = performer {
        if album.artist().is_empty() {
            album.artist = performer.to_string();
        }
    }

    for (file, mut disc) in cue.files().iter().zip(album.iter_mut()) {
        for (cue_track, mut track) in file.tracks.iter().zip(disc.iter_mut()) {
            let performer = cue_track.performer().first();
            track.set_artist(performer.cloned())
        }
    }

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        manager.add_album(album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetMusicbrainz {
    #[clap(long)]
    id: String,
    catalog: String,
}

#[handler(RepoGetMusicbrainz)]
fn repo_get_musicbrainz(
    options: RepoGetMusicbrainz,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let release = Release::fetch()
        .id(&options.id)
        .with_release_groups()
        .with_recordings()
        .with_artist_credits()
        .execute()?;
    let release_date = release
        .date
        .map(|date| AnniDate::new(date.year() as u16, date.month() as u8, date.day() as u8))
        .unwrap_or(AnniDate::UNKNOWN);
    let to_artist = |artists: Vec<ArtistCredit>| {
        artists
            .iter()
            .fold(String::new(), |acc, artist| {
                format!("{}{}、", acc, artist.name)
            })
            .trim_end_matches('、')
            .to_string()
    };
    let artist = release
        .release_group
        .and_then(|rg| rg.artist_credit)
        .map(to_artist)
        .unwrap_or_default();

    let discs = release
        .media
        .into_iter()
        .flatten()
        .map(|media| {
            let disc = DiscInfo::new(
                options.catalog.to_owned(),
                media.title,
                None,
                None,
                Default::default(),
            );

            let tracks = media
                .tracks
                .into_iter()
                .flatten()
                .map(|track| {
                    let track_type = TrackType::guess(&track.title);
                    TrackInfo::new(
                        track.title,
                        track.recording.artist_credit.map(to_artist),
                        track_type,
                        Default::default(),
                    )
                })
                .collect();
            Disc::new(disc, tracks)
        })
        .collect();

    let mut album = Album::new(
        AlbumInfo {
            title: release.title,
            artist,
            release_date,
            catalog: options.catalog,
            ..Default::default()
        },
        discs,
    );

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        manager.add_album(album, false)?;
    }
    Ok(())
}
