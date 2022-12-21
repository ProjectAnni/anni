use anni_repo::{error::Error, prelude::*, RepositoryManager};
use std::{path::PathBuf, str::FromStr};

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
                Error::RepoTagDuplicate { tag, path } => {
                    assert_eq!(tag, TagRef::new("Test", TagType::Artist));
                    assert_eq!(path, PathBuf::from_str("tag/default.toml").unwrap());
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
        artist.names().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-artist".to_string())]
    );

    let group = manager
        .tag(&TagRef::new("Test", TagType::Group))
        .expect("Tag `group:Test` does not exist");
    assert_eq!(group.name(), "Test");
    assert_eq!(
        group.names().collect::<Vec<_>>(),
        vec![(&"zh-cn".to_string(), &"Test-group".to_string())]
    );
}
