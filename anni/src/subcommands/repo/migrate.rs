use anni_metadata::annim::query::album::TagTypeInput;
use anni_metadata::annim::AnnimClient;
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
                    anni_metadata::model::TagType::Artist => TagTypeInput::Artist,
                    anni_metadata::model::TagType::Group => TagTypeInput::Group,
                    anni_metadata::model::TagType::Animation => TagTypeInput::Animation,
                    anni_metadata::model::TagType::Series => TagTypeInput::Series,
                    anni_metadata::model::TagType::Project => TagTypeInput::Project,
                    anni_metadata::model::TagType::Radio => TagTypeInput::Radio,
                    anni_metadata::model::TagType::Game => TagTypeInput::Game,
                    anni_metadata::model::TagType::Organization => TagTypeInput::Organization,
                    anni_metadata::model::TagType::Category => TagTypeInput::Category,
                    anni_metadata::model::TagType::Unknown => TagTypeInput::Others,
                },
            )
            .await?;
        if let Some(annim_tag) = annim_tag {
            log::info!("Inserted tag {}, id = {}", tag.name(), annim_tag.id.inner());
            ids.insert(tag.get_owned_ref(), annim_tag.id);
        } else {
            log::warn!("Failed to insert tag: {}", tag.name());
        }
    }
    log::info!("Finished tag insertion.");

    log::info!("Start inserting tag relation...");
    for (tag, id) in ids.iter() {
        for parent in repo.tag(tag).unwrap().parents() {
            if let Some(parent_id) = ids.get(parent) {
                let _relation = client.add_tag_relation(id, parent_id).await?;
                log::info!("Inserted tag relation {} -> {}", tag.name(), parent.name());
            }
        }
    }
    log::info!("Finished tag relation insertion.");

    let mut albums = HashMap::new();
    log::info!("Start inserting albums...");
    for album in repo.albums_iter() {
        let annim_album = client.add_album(album).await?;
        if let Some(album) = annim_album {
            log::info!("Inserted album {}, id = {}", album.title, album.id.inner());
            albums.insert(album.album_id, album);
        } else {
            log::warn!("Failed to insert album: {}", album.full_title());
        }
    }
    log::info!("Finished album insertion.");

    log::info!("Start inserting album tags...");
    for album in repo.albums_iter() {
        let album_annim = albums.get(&album.album_id()).unwrap();
        let tags = album
            .album_tags()
            .iter()
            .filter_map(|tag| ids.get(tag))
            .collect::<Vec<_>>();
        if !tags.is_empty() {
            client.set_album_tags(&album_annim.id, tags).await?;
        }
        for (index, disc) in album.iter().enumerate() {
            let disc_annim = &album_annim.discs[index];
            let tags = disc
                .tags_iter()
                .filter_map(|tag| ids.get(tag))
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                client.set_disc_tags(&disc_annim.id, tags).await?;
            }

            for (index, track) in disc.iter().enumerate() {
                let track_annim = &disc_annim.tracks[index];
                let tags = track
                    .tags_iter()
                    .filter_map(|tag| ids.get(tag))
                    .collect::<Vec<_>>();
                if !tags.is_empty() {
                    client.set_track_tags(&track_annim.id, tags).await?;
                }
            }
        }
        log::info!("Inserted album tags for album {}", album.full_title());
    }
    log::info!("Finished album tag insertion.");

    Ok(())
}
