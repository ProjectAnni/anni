# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.1

- `load_albums` now return error on tag resolve failure instead of panic
- Add `AnniDate::to_short_string` to print date in `YYMMDD` format

## 0.3.2

- [Deprecation] Rename `RepoDatabaseRead::get_tag` to `get_item_tags`
- Added `RepoDatabaseRead::get_tag_relationship`
- Removed `RepoDatabase::get_tags`, a never-used method

## 0.3.1

- impl `ToString` for `Album`
- Upgrade `toml` and `toml_edit`

## 0.3.0

- [Breaking] Change signature of `DiscInfo::new` and `Track::new`
- `Album::format` now works as expected
- Added `UNKNOWN_ARTIST` constant.

## 0.2.1

- Fix build when `search` feature is used

## 0.2.0

- [Breaking] Remove `From<&Album> for serde_json::Value`, add new `JsonAlbum` for json exchange format under `json`
- Upgrade `lindera-tantivy` to `0.23.0`. Use `ipadic-compress` by default.
- Upgrade `tantivy` to `0.19.2`
- Upgrade `git2` to `0.16.1`
- Fix tag type check constraint defined in `repo_tag` table
  feature
- Use `toml` instead of deprecated `toml_edit::easy`
- Add `apply` feature to enable `apply` method in `Album`
