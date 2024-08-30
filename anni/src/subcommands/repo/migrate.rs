use anni_metadata::mutation::add_album::{
    AddAlbumInput, CreateAlbumDiscInput, CreateAlbumTrackInput,
};
use anni_metadata::query::album::{TagType, TrackType};
use anni_metadata::AnnimClient;
use anni_repo::RepositoryManager;
use clap::Args;
use clap_handler::handler;
use std::collections::HashMap;

#[derive(Args, Debug, Clone)]
pub struct RepoMigrateAction {
    endpoint: String,
}

#[handler(RepoMigrateAction)]
async fn repo_migrate(me: RepoMigrateAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let repo = manager.into_owned_manager()?;
    let client = AnnimClient::new(me.endpoint, Some("114514"));

    // insert tags
    let mut ids = HashMap::new();
    log::info!("Start inserting tags...");
    for tag in repo.tags_iter() {
        let annim_tag = client
            .add_tag(
                tag.name().to_string(),
                match tag.tag_type() {
                    anni_repo::models::TagType::Artist => TagType::Artist,
                    anni_repo::models::TagType::Group => TagType::Group,
                    anni_repo::models::TagType::Animation => TagType::Animation,
                    anni_repo::models::TagType::Series => TagType::Series,
                    anni_repo::models::TagType::Project => TagType::Project,
                    anni_repo::models::TagType::Radio => TagType::Radio,
                    anni_repo::models::TagType::Game => TagType::Game,
                    anni_repo::models::TagType::Organization => TagType::Organization,
                    anni_repo::models::TagType::Category => TagType::Category,
                    anni_repo::models::TagType::Unknown => TagType::Others,
                },
            )
            .await?;
        if let Some(annim_tag) = annim_tag {
            log::info!("Inserted tag {}, id = {}", tag.name(), annim_tag.id.inner());
            ids.insert(tag, annim_tag.id);
        } else {
            log::warn!("Failed to insert tag: {}", tag.name());
        }
    }
    log::info!("Finished tag insertion.");

    log::info!("Start inserting albums...");
    for album in repo.albums_iter() {
        let discs: Vec<_> = album.iter().collect();
        let annim_album = client
            .add_album(AddAlbumInput {
                album_id: Some(album.album_id()),
                title: album.title_raw(),
                edition: album.edition(),
                catalog: Some(album.catalog()),
                artist: album.artist(),
                year: album.release_date().year() as i32,
                month: album.release_date().month().map(|r| r as i32),
                day: album.release_date().day().map(|r| r as i32),
                extra: None,
                discs: discs
                    .iter()
                    .map(|disc| CreateAlbumDiscInput {
                        title: disc.title_raw(),
                        catalog: Some(disc.catalog()),
                        artist: disc.artist_raw(),
                        tracks: disc
                            .iter()
                            .map(|track| CreateAlbumTrackInput {
                                title: track.title(),
                                artist: track.artist(),
                                type_: match track.track_type() {
                                    anni_repo::models::TrackType::Normal => TrackType::Normal,
                                    anni_repo::models::TrackType::Instrumental => {
                                        TrackType::Instrumental
                                    }
                                    anni_repo::models::TrackType::Absolute => TrackType::Absolute,
                                    anni_repo::models::TrackType::Drama => TrackType::Drama,
                                    anni_repo::models::TrackType::Radio => TrackType::Radio,
                                    anni_repo::models::TrackType::Vocal => TrackType::Vocal,
                                },
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .await?;
        if let Some(album) = annim_album {
            log::info!("Inserted album {}, id = {}", album.title, album.id.inner());
        } else {
            log::warn!("Failed to insert album: {}", album.full_title());
        }
    }
    log::info!("Finished album insertion.");

    // TODO: insert tag relation
    // TODO: insert album tags

    Ok(())
}
