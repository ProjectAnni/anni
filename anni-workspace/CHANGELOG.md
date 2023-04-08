# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Added `AnniWorkspace::new` to quickly find a workspace from `current_dir`
- Added `AnniWorkspace::open` to open a workspace from a path without checking its parents recursively

## 0.2.1

- Use `fs::move_dir` in publish for cross-filesystem move

## 0.2.0

- Upgrade to `anni-repo` 0.3.0

## 0.1.0

- Use `toml` instead of deprecated `toml_edit::easy`
