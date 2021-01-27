use toml::value::Datetime;
use serde::{Deserialize, Deserializer, de};
use std::marker::PhantomData;
use serde::export::fmt;
use std::str::FromStr;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Album {
    #[serde(rename = "album")]
    album_info: AlbumInfo,
    catalog: Catalog,
    discs: Vec<Disc>,
}

impl FromStr for Album {
    type Err = toml::de::Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        toml::from_str(toml_str)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct AlbumInfo {
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_or_seq_string")]
    artist: Option<Vec<String>>,
    #[serde(rename = "date")]
    release_date: Datetime,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Catalog {
    anime: Option<String>,
    #[serde(rename = "fanclub-limited")]
    fanclub_limited: Option<String>,
    #[serde(rename = "full-limited")]
    full_limited: Option<String>,
    limited: Option<String>,
    regular: Option<String>,
    all: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Disc {
    catalog: Option<String>,
    tracks: Vec<Track>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Track {
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_or_seq_string")]
    artist: Option<Vec<String>>,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
}

#[derive(Debug, PartialEq)]
enum TrackType {
    Normal,
    OffVocal,
    Instrumental,
    Drama,
    Radio,
    Other(String),
}

impl<'de> Deserialize<'de> for TrackType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "normal" => TrackType::Normal,
            "off-vocal" => TrackType::OffVocal,
            "instrumental" => TrackType::Instrumental,
            "drama" => TrackType::Drama,
            "radio" => TrackType::Radio,
            _ => TrackType::Other(s),
        })
    }
}

fn string_or_seq_string<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where D: Deserializer<'de>
{
    struct StringOrVec(PhantomData<Option<Vec<String>>>);

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where E: de::Error
        {
            Ok(Some(vec![value.to_owned()]))
        }

        fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
            where S: de::SeqAccess<'de>
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

#[cfg(test)]
mod tests {
    use crate::album::{Album, AlbumInfo, Catalog, TrackType, Disc, Track};
    use toml::value::Datetime;
    use std::str::FromStr;

    #[test]
    pub fn template_album() {
        let toml_str = r#"
[album]
# 专辑名
title = "夏凪ぎ/宝物になった日"
# 专辑歌手
# 表示专辑在显示时的归属
artist = "やなぎなぎ"
# 发行日期
date = 2020-12-16
# 音乐类型
# normal(默认): 有人声的歌曲
# off-vocal: 无人声的伴奏
# instrumental: 乐器伴奏
# drama: 以人声为主的单元剧
# radio: 以人声为主的广播节目
type = "normal"

[catalog]
# 当同一张专辑发售了不同版本时 记录不同版本的 Catalog 信息
#
# アニメ盤
# anime = "TEST-0001"
#
# FC 限定盘
# fanclub-limited = "TEST-0010"
#
# 完全生产限定盘
# full-limited = "TEST-0011"
#
# 初回限定盘
# limited = "TEST-0100"
#
# 通常盘
# regular = "TEST-0101"
#
# 当只有一个版本时 通过 all 表示
# all = "TEST-1000"
#
# 当存在多张光盘时 使用 ~ 表示连续编号
# all = "TEST-1000~1010"
all = "KSLA-0178"

# 描述某张光盘的信息
[[discs]]
# 当前光盘的盘号
catalog = "KSLA-0178"

# 描述各曲目信息
[[discs.tracks]]
title = "夏凪ぎ"
artist = "やなぎなぎ"

[[discs.tracks]]
title = "宝物になった日"
# 当歌手和专辑信息中相同时
# 可省略单曲信息中的 artist

[[discs.tracks]]
title = "夏凪ぎ(Episode 9 Ver.)"

[[discs.tracks]]
title = "宝物になった日(Episode 5 Ver.)"

[[discs.tracks]]
title = "夏凪ぎ(Instrumental)"
artist = "麻枝准"
# 单曲类型覆盖专辑音乐类型
type = "instrumental"

[[discs.tracks]]
title = "宝物になった日(Instrumental)"
artist = "麻枝准"
type = "instrumental"
"#;
        let decoded: Album = toml::from_str(toml_str).unwrap();
        assert_eq!(decoded, Album {
            album_info: AlbumInfo {
                title: "夏凪ぎ/宝物になった日".to_string(),
                artist: Some(vec!["やなぎなぎ".to_string()]),
                release_date: Datetime::from_str("2020-12-16").unwrap(),
                track_type: Some(TrackType::Normal),
            },
            catalog: Catalog {
                anime: None,
                fanclub_limited: None,
                full_limited: None,
                limited: None,
                regular: None,
                all: Some("KSLA-0178".to_string()),
            },
            discs: vec![
                Disc {
                    catalog: Some("KSLA-0178".to_string()),
                    tracks: vec![
                        Track {
                            title: "夏凪ぎ".to_string(),
                            artist: Some(vec!["やなぎなぎ".to_string()]),
                            track_type: None,
                        },
                        Track {
                            title: "宝物になった日".to_string(),
                            artist: None,
                            track_type: None,
                        },
                        Track {
                            title: "夏凪ぎ(Episode 9 Ver.)".to_string(),
                            artist: None,
                            track_type: None,
                        },
                        Track {
                            title: "宝物になった日(Episode 5 Ver.)".to_string(),
                            artist: None,
                            track_type: None,
                        },
                        Track {
                            title: "夏凪ぎ(Instrumental)".to_string(),
                            artist: Some(vec!["麻枝准".to_string()]),
                            track_type: Some(TrackType::Instrumental),
                        },
                        Track {
                            title: "宝物になった日(Instrumental)".to_string(),
                            artist: Some(vec!["麻枝准".to_string()]),
                            track_type: Some(TrackType::Instrumental),
                        },
                    ],
                }
            ],
        })
    }
}