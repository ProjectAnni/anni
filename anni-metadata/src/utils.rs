// https://github.com/serde-rs/serde/issues/1425#issuecomment-439729881
pub fn non_empty_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    use serde::Deserialize;
    let o: Option<String> = Option::deserialize(d)?;
    Ok(o.filter(|s| !s.is_empty()))
}
