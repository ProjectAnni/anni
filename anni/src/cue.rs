use cue_sheet::tracklist::Tracklist;
use shell_escape::escape;
use std::io;
use std::path::{Path, PathBuf};
use clap::ArgMatches;
use anni_utils::fs;
use std::str::FromStr;

pub(crate) fn handle_cue(matches: &ArgMatches) -> anyhow::Result<()> {
    let (cue, files) = if matches.is_present("cue.file") {
        // In file mode, the path of CUE file is specified by -f
        // And all the files in <Filename> are FLAC files
        let c = matches.value_of("cue.file")
            .map(|u| PathBuf::from_str(u)) // map file path to PathBuf
            .unwrap()?; // match must present
        let f = matches.values_of("Filename")
            .ok_or(anyhow!("No FLAC file provided."))?
            .map(|u| PathBuf::from_str(u)).collect::<Result<Vec<_>, _>>()?;
        (c, f)
    } else if matches.is_present("cue.dir") && matches.is_present("Filename") {
        // In directory mode, only one path is used: <Filename>[0]
        // The first CUE file found in that directory is treated as CUE input
        // All other FLAC file in that directory are treated as input
        let dir = matches.value_of("Filename").ok_or(anyhow!("No filename provided."))?;
        let c = fs::get_ext_file(dir, "cue", false)?.ok_or(anyhow!("No CUE file found."))?;
        let f = fs::get_ext_files(PathBuf::from(dir), "flac", false)?
            .ok_or(anyhow!("No FLAC file found"))?;
        // .map(|p| p.iter().map(|t| t.to_str().unwrap().to_owned()).collect::<Vec<_>>());
        (c, f)
    } else {
        unimplemented!();
    };

    if matches.is_present("cue.tagsh") {
        let result = parse_file(cue, &files)?;
        println!("{}", result);
    }
    Ok(())
}

fn parse_file<P: AsRef<Path>>(path: P, files: &[P]) -> anyhow::Result<String> {
    let mut str: &str = &fs::read_to_string(path)?;

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
        result += &format!("echo {} | metaflac --remove-all-tags --import-tags-from=- {}", escape(meta.into()), escape(files[i].as_ref().to_string_lossy()));
        result.push('\n');
    }
    Ok(result)
}

pub(crate) struct CueTrack {
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

fn tracks(file: &str) -> io::Result<Vec<String>> {
    let cue = Tracklist::parse(file).unwrap();
    let album = cue.info.get("TITLE").expect("Album TITLE not provided!");
    let artist = cue.info.get("ARTIST").map(String::as_str).unwrap_or("");
    let date = cue.info.get("DATE").expect("Album DATE not provided!");
    let disc_number = cue.info.get("DISCNUMBER").map(String::as_str).unwrap_or("1");
    let disc_total = cue.info.get("TOTALDISCS").map(String::as_str).unwrap_or("1");

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