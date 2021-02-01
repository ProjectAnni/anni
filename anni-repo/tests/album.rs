use anni_repo::album::{Album, TrackType};
use std::str::FromStr;

#[test]
fn decode_album_toml() {
    let album = Album::from_str(r#"
[album]
# 专辑名
title = "夏凪ぎ/宝物になった日"
# 专辑歌手 表示专辑在显示时的归属
artist = "やなぎなぎ"
# 发行日期
date = 2020-12-16
# 音乐类型
# normal(默认): 有人声的歌曲
# instrumental: 无人声的伴奏
# absolute: 纯音乐
# drama: 以人声为主的单元剧
# radio: 以人声为主的广播节目
type = "normal"
# 通过 catalog 表示光盘的盘号
# 当存在多张光盘时 使用 ~ 表示连续编号
catalog = "KSLA-0178"

# 描述某张光盘的信息
[[discs]]
# 当前光盘的盘号
catalog = "KSLA-0178"

# 描述各曲目信息
[[discs.tracks]]
title = "夏凪ぎ"
# 当歌手和专辑信息中相同时
# 可省略单曲信息中的 artist
# 下行的 artist 可省略
artist = "やなぎなぎ"
# 文本歌词
lyric = "[KSLA-0178] 夏凪ぎ.txt"

[[discs.tracks]]
title = "宝物になった日"
# lrc 歌词
lyric = "[KSLA-0178] 宝物になった日.lrc"

[[discs.tracks]]
title = "夏凪ぎ(Episode 9 Ver.)"
# 指定 lrc 歌词的偏移时间(ms)
lyric = { file = "[KSLA-0178] 夏凪ぎ.lrc", offset = 100 }

[[discs.tracks]]
title = "宝物になった日(Episode 5 Ver.)"
lyric = { file = "[KSLA-0178] 宝物になった日.lrc", offset = 100 }

[[discs.tracks]]
title = "夏凪ぎ(Instrumental)"
# 当歌手和专辑信息中不同时
# Track 内信息覆盖全局信息
artist = "麻枝准"
# 单曲类型覆盖专辑音乐类型
type = "instrumental"

[[discs.tracks]]
title = "宝物になった日(Instrumental)"
artist = "麻枝准"
type = "instrumental"
"#).expect("Failed to parse album toml.");

    assert_eq!(album.title(), "夏凪ぎ/宝物になった日");
    assert_eq!(album.artist(), "やなぎなぎ");
    assert_eq!(album.release_date(), "2020-12-16");
    assert_eq!(album.track_type().to_string(), "normal");
    assert_eq!(album.catalog(), "KSLA-0178");

    for disc in album.discs() {
        assert_eq!(disc.catalog(), "KSLA-0178");
        for (i, track) in disc.tracks().iter().enumerate() {
            match i {
                0 => {
                    assert_eq!(track.title(), "夏凪ぎ");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert_eq!(track.track_type(), TrackType::Normal);
                }
                1 => {
                    assert_eq!(track.title(), "宝物になった日");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert_eq!(track.track_type(), TrackType::Normal);
                }
                2 => {
                    assert_eq!(track.title(), "夏凪ぎ(Episode 9 Ver.)");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert_eq!(track.track_type(), TrackType::Normal);
                }
                3 => {
                    assert_eq!(track.title(), "宝物になった日(Episode 5 Ver.)");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert_eq!(track.track_type(), TrackType::Normal);
                }
                4 => {
                    assert_eq!(track.title(), "夏凪ぎ(Instrumental)");
                    assert_eq!(track.artist(), "麻枝准");
                    assert_eq!(track.track_type(), TrackType::Instrumental);
                }
                5 => {
                    assert_eq!(track.title(), "宝物になった日(Instrumental)");
                    assert_eq!(track.artist(), "麻枝准");
                    assert_eq!(track.track_type(), TrackType::Instrumental);
                }
                _ => unreachable!(),
            }
        }
    }
}