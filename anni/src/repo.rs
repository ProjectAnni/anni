use anni_repo::Datetime;
use std::str::FromStr;

fn files_to_repo_album(files: &[anni_flac::Stream]) -> Result<anni_repo::Album, Box<dyn std::error::Error>> {
    if files.len() == 0 {
        return Err("No file provided.".into());
    }
    let first = files[0].comments().ok_or("Failed to get comments")?;
    let mut album = anni_repo::Album::new(
        first["ALBUM"].value(),
        first["ARTIST"].value(),
        Datetime::from_str("2021-01-01")?, // TODO
        "CATA-001", // TODO
    );
    let mut disc = anni_repo::album::Disc::new();
    for file in files {
        let comment = file.comments().ok_or("No comments found.")?;
        disc.add_track(anni_repo::album::Track::new(comment["TITLE"].value(), Some(comment["ARTIST"].value()), None));
    }
    album.add_disc(disc);
    Ok(album)
}