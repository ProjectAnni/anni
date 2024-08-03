# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.1

- Upgrade `anni-common` to `0.2.0`

## 0.3.0

- **[Breaking]** Change definition of `AudioInfo::duration`. Now this value uses milliseconds instead of seconds.
- Added `PriorityProvider` and `priority` feature
- Upgrade `anni-common` to 0.1.4

## 0.2.0

- Upgrade `anni-repo` to `0.3.0`

## 0.1.3

- Upgrade `lru` to `0.10.0`
- Upgrade `anni-repo` to `0.2.0`

## 0.1.2

- Added `MultipleProviders`, allows user to combine multiple `AnniProviders` and serve as a whole.
- Implemented `AnniProvider::album` for `NoCacheStrictLocalProvider` correctly.
- Fixed `range` returned by `NoCacheStrictLocalProvider::get_audio`.
