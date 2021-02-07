use std::path::Path;
use crate::Datetime;
use regex::Regex;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn file_name<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        path.as_ref().canonicalize()?
    };
    Ok(path.file_name()
        .ok_or("No filename found")?
        .to_str().ok_or("Invalid UTF-8 path")?
        .to_owned())
}

pub fn parts_to_date(y: &str, m: &str, d: &str) -> Result<Datetime> {
    // yyyy-mm-dd
    let mut date = String::with_capacity(10);
    // for yymmdd
    if y.len() == 2 {
        date += if u8::from_str(y)? > 30 {
            "19"
        } else {
            "20"
        }
    }
    date += y;
    date += "-";
    date += m;
    date += "-";
    date += d;
    Ok(Datetime::from_str(&date)?)
}

pub fn disc_info(path: &str) -> Result<(String, String, usize)> {
    let r = Regex::new(r"^\[([^]]+)] (.+) \[Disc (\d+)]$")?;
    let r = r.captures(path).ok_or("No capture found")?;
    if r.len() == 0 {
        return Err("".into());
    }

    Ok((
        r.get(1).unwrap().as_str().to_owned(),
        r.get(2).unwrap().as_str().to_owned(),
        usize::from_str(r.get(3).unwrap().as_str())?))
}

pub fn album_info(path: &str) -> Result<(Datetime, String, String)> {
    let r = Regex::new(r"^\[(\d{2}|\d{4})(\d{2})(\d{2})]\[([^]]+)] (.+)$")?;
    let r = r.captures(path).ok_or("No capture found")?;
    if r.len() == 0 {
        return Err("".into());
    }

    Ok((
        parts_to_date(
            r.get(1).unwrap().as_str(),
            r.get(2).unwrap().as_str(),
            r.get(3).unwrap().as_str(),
        )?,
        r.get(4).unwrap().as_str().to_owned(),
        r.get(5).unwrap().as_str().to_owned()))
}