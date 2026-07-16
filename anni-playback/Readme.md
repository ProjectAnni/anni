# anni-playback

An audio playback library built on Symphonia, Rubato, and CPAL. It provides a
low-level configurable `Player` and an annil-aware `AnniPlayer` with variant-safe
on-disk caching.

## Configurable player

```rust,no_run
use std::time::Duration;
use anni_playback::{DecodeSettings, Player, PlayerEvent};

# fn main() -> anyhow::Result<()> {
let (player, events) = Player::builder()
    .buffer_duration(Duration::from_millis(750))
    .preferred_sample_rate(Some(48_000))
    .decode_settings(DecodeSettings {
        gapless: true,
        verify: false,
        recover_decode_errors: true,
        max_consecutive_errors: 8,
        ..Default::default()
    })
    .build()?;

player.open_file("track.flac", false)?;
player.play();

while let Ok(event) = events.recv() {
    match event {
        PlayerEvent::Error(error) => eprintln!("{error:?}"),
        PlayerEvent::Stop => break,
        _ => {}
    }
}

let stats = player.stats();
println!(
    "buffer={}ms underruns={} decoded_frames={}",
    stats.buffered_duration_ms(),
    stats.underruns,
    stats.decoded_frames,
);
# Ok(())
# }
```

`build()` opens the output device immediately and returns device/configuration
errors to the caller. `build_lazy()` defers that work until `play()`, which is
useful for headless decoding or applications that may start before an audio
device is available; later output failures arrive as `PlayerEvent::Error`.

`PlayerConfig` is split into `OutputSettings`, `DecodeSettings`, and
`PreloadSettings`. The cheap snapshot returned by `stats()` includes buffer
occupancy, source-vs-output buffering, underruns, dropped/output samples,
decoded and preloaded packets/frames, recoverable decode errors, and the actual
source/output formats.

## Preload and gapless playback

Queue the next source with `open_file(path, true)` (or `AnniPlayer::preload`).
The decoder prepares multiple packets until `PreloadSettings::target_duration`
is satisfied and emits `PreloadReady`. At the current track's natural end it
flushes the exact resampler tail and appends the next track to the same PCM
ring; each track still receives a fresh converter, so sample-rate/channel
changes cannot reuse stale DSP state. `play_preloaded()` performs an immediate
manual switch and intentionally discards the current buffered tail.

## annil player

`AnniPlayer::builder(provider, cache_path)` accepts the same playback settings,
an optional HTTP client, and a network timeout. Use `AudioVariant` to select a
codec and quality explicitly. Cache entries include both values, so AAC, Opus,
and lossless representations cannot alias each other.

annil currently maps `Low`, `Medium`, and `High` to 128, 192, and 256 kbps;
`AudioQuality::bitrate_kbps()` exposes that mapping. `open_variant()` and
`preload()` return the effective `AudioVariant`, because annil may enforce a
different quality for guest access. Cache files are keyed by that effective
quality and the GET response codec (rather than trusting HEAD alone), validated
before publication, and incomplete downloads remain private `.part` files.

The legacy `Controls`/`Decoder` construction API and the legacy
`AnniPlayer::new` constructor remain available for compatibility. New code
should prefer the builders because initialization errors are returned instead
of being hidden on the decoder thread.
