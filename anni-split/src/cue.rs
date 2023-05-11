use crate::{codec::wav::WaveHeader, split::Breakpoint};

pub struct CueBreakpoint {
    seconds: u32,
    frames: u32,
}

impl Breakpoint for CueBreakpoint {
    fn position(&self, header: &WaveHeader) -> u32 {
        header.offset_from_second_frames(self.seconds, self.frames)
    }
}

pub fn cue_breakpoints<C>(cue: C) -> impl IntoIterator<Item = CueBreakpoint>
where
    C: AsRef<str>,
{
    let cue = cuna::Cuna::new(cue.as_ref()).unwrap();

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

    result
}
