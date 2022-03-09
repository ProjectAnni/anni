use anni_repo::prelude::*;
use std::str::FromStr;

fn repo_from_str() -> Repository {
    Repository::from_str(
        r#"
[repo]
name = "Yesterday17's Metadata Repo"
edition = "1.3"
"#,
    )
    .expect("Failed to parse toml")
}

#[test]
fn deserialize_repo_toml() {
    let repo = repo_from_str();
    assert_eq!(repo.name(), "Yesterday17's Metadata Repo");
    assert_eq!(repo.edition(), "1.3");
}

#[test]
fn serialize_repo() {
    assert_eq!(
        repo_from_str().to_string(),
        r#"[repo]
name = "Yesterday17's Metadata Repo"
edition = "1.3"
"#
    );
}
