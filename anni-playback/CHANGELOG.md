# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Make `decoder::CODEC_REGISTRY` public
- Upgraded `ratatui` used by example
- Added configurable `Player` and `AnniPlayer` builders plus playback/cache statistics
- Fixed partial ring-buffer reads, device format negotiation, and mono/multichannel output mapping
- Added a software output gate so pause works on backends without hardware stream pausing
- Added decoder-confirmed state/error events and deterministic shutdown/stop behavior
- Made cache keys codec/quality aware and cache completion atomic
- Added bounded multi-packet preloading and exact resampler delay/padding trimming
- Added sample-accurate seek trimming and separate source/output buffering statistics
- Key annil cache entries by the GET-resolved bitrate/codec and validate downloads before publish
