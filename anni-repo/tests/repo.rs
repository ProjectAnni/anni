use anni_repo::repo::{Repository};
use std::str::FromStr;

#[test]
fn deserialize_repo_toml() {
    let repo = Repository::from_str(r#"
[repo]
# 仓库名
name = "Yesterday17's Metadata Repo"
# 仓库文件版本 每次提交需要更新
version = "1.0.0+1"
# 仓库维护者
authors = ["Yesterday17 <t@yesterday17.cn>"]
# 仓库使用的元数据仓库描述版本
edition = "1"

[repo.cover]
# 启用仓库封面
enable = true
# 存放封面文件的地址
root = "//example-cover-root/"

[repo.lyric]
# 启用仓库歌词
enable = true
# 存放歌词文件的地址
root = "//example-lyric-root/"
"#).expect("Failed to parse toml");

    assert_eq!(repo.name(), "Yesterday17's Metadata Repo");
    assert_eq!(repo.version(), "1.0.0+1");
    assert_eq!(repo.authors(), vec!["Yesterday17 <t@yesterday17.cn>"]);
    assert_eq!(repo.edition(), "1");

    match repo.cover() {
        Some(cover) => {
            assert_eq!(cover.enable, true);
            assert_eq!(cover.root(), Some("//example-cover-root/"));
        }
        None => unreachable!(),
    }

    match repo.lyric() {
        Some(lyric) => {
            assert_eq!(lyric.enable, true);
            assert_eq!(lyric.root(), Some("//example-lyric-root/"));
        }
        None => unreachable!(),
    }
}