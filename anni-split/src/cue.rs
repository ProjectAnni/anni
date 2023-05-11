use crate::error::SplitError;
use crate::{codec::wav::WaveHeader, split::Breakpoint};
use cuna::Cuna;

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
    }) = result.get(0)
    {
        result.remove(0);
    }

    Ok((result, cue))
}
