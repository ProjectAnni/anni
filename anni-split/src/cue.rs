use crate::error::SplitError;
use crate::{codec::wav::WaveHeader, split::Breakpoint};
use cuna::Cuna;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRange {
    track_number: u8,
    cue_index_offset: u32,
    start: u32,
    end: u32,
    title: Option<String>,
}

impl TrackRange {
    pub const fn track_number(&self) -> u8 {
        self.track_number
    }

    pub const fn cue_index_offset(&self) -> u32 {
        self.cue_index_offset
    }

    pub const fn start(&self) -> u32 {
        self.start
    }

    pub const fn end(&self) -> u32 {
        self.end
    }

    pub const fn byte_length(&self) -> u32 {
        self.end - self.start
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// Validated byte ranges for one WAV file described by one CUE `FILE` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueSplitPlan {
    source_file: String,
    tracks: Vec<TrackRange>,
}

impl CueSplitPlan {
    pub fn new(cue: &str, header: &WaveHeader) -> Result<Self, CueSplitPlanError> {
        if header.block_align == 0 || header.byte_rate == 0 || header.data_size == 0 {
            return Err(CueSplitPlanError::InvalidWaveHeader);
        }
        if !header
            .data_size
            .is_multiple_of(u32::from(header.block_align))
        {
            return Err(CueSplitPlanError::MisalignedDataSize {
                data_size: header.data_size,
                block_align: header.block_align,
            });
        }

        let cue = Cuna::new(cue)?;
        if cue.files.len() != 1 {
            return Err(CueSplitPlanError::UnsupportedFileCount {
                actual: cue.files.len(),
            });
        }
        let file = &cue.files[0];
        if file.tracks.is_empty() {
            return Err(CueSplitPlanError::NoTracks);
        }

        let mut cue_offsets = Vec::with_capacity(file.tracks.len());
        let mut previous_track = None;
        let mut previous_offset = None;
        for track in &file.tracks {
            if let Some(previous_track) = previous_track
                && track.id() <= previous_track
            {
                return Err(CueSplitPlanError::NonIncreasingTrackNumber {
                    previous: previous_track,
                    current: track.id(),
                });
            }

            let index = track
                .get_index(1)
                .ok_or(CueSplitPlanError::MissingIndexOne { track: track.id() })?;
            let byte_offset = header.byte_offset_from_cd_frames(index.begin_time().total_frames());
            if byte_offset > u64::from(header.data_size) {
                return Err(CueSplitPlanError::OffsetOutOfBounds {
                    track: track.id(),
                    offset: byte_offset,
                    data_size: header.data_size,
                });
            }
            let byte_offset =
                u32::try_from(byte_offset).expect("offset bounded by the u32 WAV data size");
            if byte_offset % u32::from(header.block_align) != 0 {
                return Err(CueSplitPlanError::MisalignedOffset {
                    track: track.id(),
                    offset: byte_offset,
                    block_align: header.block_align,
                });
            }
            if let Some(previous_offset) = previous_offset
                && byte_offset <= previous_offset
            {
                return Err(CueSplitPlanError::NonIncreasingOffset {
                    previous_track: previous_track.expect("previous offset has a track"),
                    track: track.id(),
                    previous_offset,
                    offset: byte_offset,
                });
            }

            cue_offsets.push((track, byte_offset));
            previous_track = Some(track.id());
            previous_offset = Some(byte_offset);
        }

        let mut tracks = Vec::with_capacity(cue_offsets.len());
        for (index, (track, cue_index_offset)) in cue_offsets.iter().enumerate() {
            let start = if index == 0 { 0 } else { *cue_index_offset };
            let end = cue_offsets
                .get(index + 1)
                .map(|(_, offset)| *offset)
                .unwrap_or(header.data_size);
            if start >= end {
                return Err(CueSplitPlanError::EmptyTrackRange {
                    track: track.id(),
                    start,
                    end,
                });
            }
            tracks.push(TrackRange {
                track_number: track.id(),
                cue_index_offset: *cue_index_offset,
                start,
                end,
                title: track.title().first().cloned(),
            });
        }

        Ok(Self {
            source_file: file.name.clone(),
            tracks,
        })
    }

    pub fn source_file(&self) -> &str {
        &self.source_file
    }

    pub fn tracks(&self) -> &[TrackRange] {
        &self.tracks
    }
}

#[derive(Debug, Error)]
pub enum CueSplitPlanError {
    #[error(transparent)]
    Cue(#[from] cuna::error::Error),
    #[error("WAV header has zero byte rate, block alignment, or data size")]
    InvalidWaveHeader,
    #[error("WAV data size {data_size} is not aligned to {block_align} bytes")]
    MisalignedDataSize { data_size: u32, block_align: u16 },
    #[error("single-input splitting requires exactly one CUE FILE block, got {actual}")]
    UnsupportedFileCount { actual: usize },
    #[error("CUE FILE block contains no tracks")]
    NoTracks,
    #[error("track {track:02} has no INDEX 01")]
    MissingIndexOne { track: u8 },
    #[error("track number {current:02} does not follow {previous:02}")]
    NonIncreasingTrackNumber { previous: u8, current: u8 },
    #[error("track {track:02} offset {offset} is beyond WAV data size {data_size}")]
    OffsetOutOfBounds {
        track: u8,
        offset: u64,
        data_size: u32,
    },
    #[error("track {track:02} offset {offset} is not aligned to {block_align} bytes")]
    MisalignedOffset {
        track: u8,
        offset: u32,
        block_align: u16,
    },
    #[error(
        "track {track:02} offset {offset} is not after track {previous_track:02} offset {previous_offset}"
    )]
    NonIncreasingOffset {
        previous_track: u8,
        track: u8,
        previous_offset: u32,
        offset: u32,
    },
    #[error("track {track:02} has empty byte range {start}..{end}")]
    EmptyTrackRange { track: u8, start: u32, end: u32 },
}

/// `Cue` files uses format like `mm:ss.ff` to describe time of tracks.
/// [CueBreakpoint] reuses this value, and can be used to split wave files, depending on its byte-rate.
pub struct CueBreakpoint {
    seconds: u32,
    frames: u32,
}

impl Breakpoint for CueBreakpoint {
    fn position(&self, header: &WaveHeader) -> u32 {
        header.offset_from_second_frames(self.seconds, self.frames)
    }
}

/// Extract breakpoints from a cue file.
/// Behavior should be the same as `--append-gaps` flag enabled in [cuebreakpoints](https://github.com/svend/cuetools/blob/master/src/tools/cuebreakpoints.c).
///
/// It returns an iterator of breakpoints, and a [Cuna] object.
pub fn cue_breakpoints<C>(
    cue: C,
) -> Result<(impl IntoIterator<Item = CueBreakpoint>, Cuna), SplitError>
where
    C: AsRef<str>,
{
    let cue = Cuna::new(cue.as_ref())?;

    if cue.files.len() != 1 {
        return Err(SplitError::UnsupportedCueFileCount {
            actual: cue.files.len(),
        });
    }

    let total_tracks = cue.files.iter().map(|f| f.tracks.len()).sum();
    let mut result = Vec::with_capacity(total_tracks);

    for file in cue.files.iter() {
        for track in file.tracks.iter() {
            for index in track.index.iter() {
                if index.id() == 1 {
                    let time = index.begin_time();
                    result.push(CueBreakpoint {
                        seconds: time.total_seconds(),
                        frames: time.frames(),
                    });
                }
            }
        }
    }

    if let Some(CueBreakpoint {
        seconds: 0,
        frames: 0,
    }) = result.first()
    {
        result.remove(0);
    }

    Ok((result, cue))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(seconds: u32) -> WaveHeader {
        WaveHeader {
            channels: 2,
            sample_rate: 44_100,
            byte_rate: 176_400,
            block_align: 4,
            bit_per_sample: 16,
            data_size: 176_400 * seconds,
        }
    }

    #[test]
    fn cue_plan_produces_aligned_ranges_and_preserves_titles() {
        let cue = r#"
FILE "album.wav" WAVE
  TRACK 01 AUDIO
    TITLE "第一曲（原文）"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Second Track"
    INDEX 01 01:00:00
"#;

        let plan = CueSplitPlan::new(cue, &header(120)).unwrap();

        assert_eq!(plan.source_file(), "album.wav");
        assert_eq!(plan.tracks().len(), 2);
        assert_eq!(plan.tracks()[0].start(), 0);
        assert_eq!(plan.tracks()[0].end(), 176_400 * 60);
        assert_eq!(plan.tracks()[0].title(), Some("第一曲（原文）"));
        assert_eq!(plan.tracks()[1].start(), 176_400 * 60);
        assert_eq!(plan.tracks()[1].end(), 176_400 * 120);
    }

    #[test]
    fn cue_plan_rejects_multiple_file_timelines() {
        let cue = r#"
FILE "disc-a.wav" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
FILE "disc-b.wav" WAVE
  TRACK 02 AUDIO
    INDEX 01 00:00:00
"#;

        assert!(matches!(
            CueSplitPlan::new(cue, &header(120)),
            Err(CueSplitPlanError::UnsupportedFileCount { actual: 2 })
        ));
        assert!(matches!(
            cue_breakpoints(cue),
            Err(SplitError::UnsupportedCueFileCount { actual: 2 })
        ));
    }

    #[test]
    fn cue_plan_rejects_non_increasing_and_out_of_bounds_offsets() {
        let non_increasing = r#"
FILE "album.wav" WAVE
  TRACK 01 AUDIO
    INDEX 01 01:00:00
  TRACK 02 AUDIO
    INDEX 01 00:30:00
"#;
        assert!(matches!(
            CueSplitPlan::new(non_increasing, &header(120)),
            Err(CueSplitPlanError::NonIncreasingOffset { .. })
        ));

        let out_of_bounds = r#"
FILE "album.wav" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    INDEX 01 03:00:00
"#;
        assert!(matches!(
            CueSplitPlan::new(out_of_bounds, &header(120)),
            Err(CueSplitPlanError::OffsetOutOfBounds { track: 2, .. })
        ));
    }
}
