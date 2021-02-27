use cue_sheet::tracklist::{Tracklist};
use shell_escape::escape;
use std::io;
use anni_utils::validator::date_validator;
use std::path::Path;

pub(crate) fn parse_file<T: AsRef<str>>(path: &str, files: &[T]) -> anyhow::Result<String> {
    let mut str: &str = &std::fs::read_to_string(path)?;

    let first = str.chars().next().ok_or(io::Error::new(io::ErrorKind::InvalidData, "Empty CUE file"))?;
    // UTF-8 BOM
    if first == '\u{feff}' {
        str = &str[3..];
    }

    let mut result = String::new();
    let tracks = tracks(str)?;
    if files.len() != tracks.len() {
        bail!("Incorrect file number. Expected {}, got {}", tracks.len(), files.len());
    }

    for (i, meta) in tracks.iter().enumerate() {
        result += &format!("echo {} | metaflac --remove-all-tags --import-tags-from=- {}", escape(meta.into()), escape(files[i].as_ref().into()));
        result.push('\n');
    }
    Ok(result)
}

pub struct CueTrack {
    pub index: u8,
    pub title: String,
    pub mm: usize,
    pub ss: usize,
    pub ff: usize,
}

pub(crate) fn extract_breakpoints<P: AsRef<Path>>(path: P) -> Vec<CueTrack> {
    let mut str: &str = &std::fs::read_to_string(path).unwrap();

    let first = str.chars().next().ok_or(io::Error::new(io::ErrorKind::InvalidData, "Empty CUE file")).unwrap();
    // UTF-8 BOM
    if first == '\u{feff}' {
        str = &str[3..];
    }

    let mut result = Vec::new();
    let cue = Tracklist::parse(str).unwrap();
    for file in cue.files.iter() {
        for (i, track) in file.tracks.iter().enumerate() {
            for (index, time) in track.index.iter() {
                if *index == 1 {
                    result.push(CueTrack {
                        index: (i + 1) as u8,
                        title: (&track.info["TITLE"]).to_owned(),
                        mm: time.minutes() as usize,
                        ss: time.seconds() as usize,
                        ff: time.frames() as usize,
                    });
                }
            }
        }
        break;
    }
    result
}

pub(crate) fn tracks(file: &str) -> io::Result<Vec<String>> {
    let cue = Tracklist::parse(file).unwrap();
    let album = cue.info.get("TITLE").expect("Album TITLE not provided!");
    let artist = cue.info.get("ARTIST").map(String::as_str).unwrap_or("");
    let date = cue.info.get("DATE").expect("Album DATE not provided!");
    let disc_number = cue.info.get("DISCNUMBER").map(String::as_str).unwrap_or("1");
    let disc_total = cue.info.get("TOTALDISCS").map(String::as_str).unwrap_or("1");

    if !date_validator(date) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid date format in cue file!"));
    }

    let mut track_number = 1;
    let mut track_total = 0;
    for file in cue.files.iter() {
        for _track in file.tracks.iter() {
            track_total += 1;
        }
    }

    let mut result = Vec::with_capacity(track_total);
    for file in cue.files.iter() {
        for track in file.tracks.iter() {
            let title = track.info.get("TITLE").expect("Track TITIE not provided!");
            let artist = track.info.get("ARTIST").map(String::as_str).unwrap_or(artist);
            assert!(artist.len() > 0);

            result.push(format!(r#"TITLE={}
ALBUM={}
ARTIST={}
DATE={}
TRACKNUMBER={}
TRACKTOTAL={}
DISCNUMBER={}
DISCTOTAL={}"#, title, album, artist, date, track_number, track_total, disc_number, disc_total));

            track_number += 1;
        }
    }
    Ok(result)
}