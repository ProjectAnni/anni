use anni_repo::{error::Error, prelude::*, RepositoryManager};
use std::str::FromStr;

fn repo_from_str() -> Repository {
    Repository::from_str(
        r#"[repo]
name = "Yesterday17's Metadata Repo"
edition = "1.3"
"#,
    )
    .expect("Failed to parse toml")
}

#[test]
fn serialize_repo() {
    assert_eq!(
        repo_from_str().to_string(),
        r#"[repo]
name = "Yesterday17's Metadata Repo"
edition = "1.3"
albums = ["album"]
"#
    );
}

#[test]
fn test_empty_repository() {
    let manager =
        RepositoryManager::new("tests/repos/empty").expect("Failed to load metadata repository");
    assert_eq!(manager.name(), "Metadata repo test cases");
    assert_eq!(manager.edition(), "1.0+alpha.1.5.1");

    manager
        .into_owned_manager()
        .expect("Failed to load empty repository");
}

#[test]
fn test_duplicated_tags() {
    let manager = RepositoryManager::new("tests/repos/duplicated-tags")
        .expect("Failed to load metadata repository");
    let result = manager.into_owned_manager();
    match result {
        Ok(_) => {
            panic!("Metadata repository with duplicated tags is not valid.");
        }
        Err(err) => {
            assert!(match err {
                Error::RepoTagDuplicated(tag) => {
                    assert_eq!(tag, TagRef::new("Test", TagType::Artist));
                    true
                }
                _ => false,
            });
        }
    }
}

#[test]
fn test_duplicated_tag_name_but_different_type() {
    let manager = RepositoryManager::new("tests/repos/duplicated-tag-name-different-type")
        .expect("Failed to load metadata repository");
    let manager = manager
        .into_owned_manager()
        .expect("Failed to initialize repository for tags with different type");

    let artist = manager
        .tag(&TagRef::new("Test", TagType::Artist))
        .expect("Tag `artist:Test` does not");
    assert_eq!(artist.name(), "Test");
    assert_eq!(
        artist.names().iter().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-artist".to_string())]
    );

    let group = manager
        .tag(&TagRef::new("Test", TagType::Group))
        .expect("Tag `group:Test` does not exist");
    assert_eq!(group.name(), "Test");
    assert_eq!(
        group.names().iter().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-group".to_string())]
    );
}

#[test]
fn test_repo_album_tags() {
    let manager = RepositoryManager::new("tests/repos/album-tags")
        .expect("Failed to load metadata repository");
    let manager = manager
        .into_owned_manager()
        .expect("Failed to initialize repository for tags with different type");

    let non_dup = manager
        .tag(&TagRef::new("Test", TagType::Artist))
        .expect("Tag `artist:Test` does not");
    assert_eq!(non_dup.name(), "Test");

    let dup_artist = manager
        .tag(&TagRef::new("Test-dup", TagType::Artist))
        .expect("Tag `artist:Test-dup` does not exist");
    assert_eq!(dup_artist.name(), "Test-dup");
    assert_eq!(
        dup_artist.names().iter().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-artist".to_string())]
    );

    let dup_group = manager
        .tag(&TagRef::new("Test-dup", TagType::Group))
        .expect("Tag `group:Test-dup` does not exist");
    assert_eq!(dup_group.name(), "Test-dup");
    assert_eq!(
        dup_group.names().iter().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-group".to_string())]
    );

    // test album
    let albums = manager.albums();
    assert_eq!(albums.len(), 1);
    let album = albums.values().next().unwrap();
    assert_eq!(album.full_title(), "Title");
    assert_eq!(album.artist(), "Artist");
    assert_eq!(album.release_date().to_string(), "2999-12-31");
    assert_eq!(album.track_type(), &TrackType::Normal);
    assert_eq!(album.catalog(), "album");
    assert_eq!(
        album.tags(),
        vec![
            &TagRef::new("Test", TagType::Artist),
            &TagRef::new("Test-dup", TagType::Artist),
            &TagRef::new("Test-dup", TagType::Group),
        ]
    );
    let discs: Vec<_> = album.iter().collect();
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].catalog(), "TEST-0001");
    let tracks: Vec<_> = discs[0].iter().collect();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].title(), "Track 1");
    assert_eq!(tracks[0].artist(), "Artist1");
    assert_eq!(tracks[0].track_type(), &TrackType::Absolute);
}
