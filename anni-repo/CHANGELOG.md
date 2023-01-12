# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Fix tag type check constraint defined in `repo_tag` table
- [Breaking] Remove `From<&Album> for serde_json::Value`, add new `JsonAlbum` for json exchange format under `json` feature
