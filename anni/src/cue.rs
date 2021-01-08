use cue_sheet::tracklist::{Tracklist};
use std::fs::File;
use std::path::PathBuf;
use shell_escape::escape;

pub(crate) fn parse_file(path: &str, files: &[&str]) -> Option<String> {
    let mut str: &str = &std::fs::read_to_string(path).ok()?;
    let first = str.chars().next().unwrap();
    if first == '\u{feff}' {
        // UTF-8 BOM
        str = &str[3..];
    }

    let mut result = String::new();
    let tracks = tracks(str);
    if files.len() != tracks.len() {
        return None;
    }

    for (i, meta) in tracks.iter().enumerate() {
        result += &format!("echo {} | metaflac --remove-all-tags --import-tags-from=- {}", escape(meta.into()), escape(files[i].into()));
        result.push('\n');
    }
    Some(result)
}

pub(crate) fn tracks(file: &str) -> Vec<String> {
    let empty_string = String::new();
    let one = String::from("1");

    let cue = Tracklist::parse(file).unwrap();
    let album = cue.info.get("TITLE").expect("Album TITLE not provided!");
    let artist = cue.info.get("ARTIST").unwrap_or(&empty_string);
    let date = cue.info.get("DATE").expect("Album DATE not provided!");
    let disc_number = cue.info.get("DISCNUMBER").unwrap_or(&one);
    let disc_total = cue.info.get("TOTALDISCS").unwrap_or(&one);

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
            let artist = track.info.get("ARTIST").unwrap_or(artist);
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
    result
}

#[cfg(test)]
mod tests {
    use crate::cue::tracks;

    #[test]
    fn track_test() {
        tracks(r#"TITLE "ご注文はうさぎですか? キャラクターソング・セレクションアルバム order the songs [Disc 1]"
REM GENRE "Soundtrack"
REM DATE "2016"
REM DISCNUMBER 1
REM TOTALDISCS 2
REM DISCID A10DAB0E
REM COMPILATION TRUE
FILE "GNCA-1501-1.flac" WAVE
  TRACK 01 AUDIO
    TITLE "宝箱のジェットコースター"
    PERFORMER "Petit Rabbit's (佐倉綾音, 水瀬いのり, 種田梨沙, 佐藤聡美, 内田真礼)"
    SONGWRITER "大久保薫"
    ISRC JPPI01551540
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "VSマイペース?"
    PERFORMER "振り回され隊 (水瀬いのり, 種田梨沙, 内田真礼)"
    SONGWRITER "大川茂伸"
    ISRC JPPI01452670
    INDEX 00 03:23:73
    INDEX 01 03:24:73
  TRACK 03 AUDIO
    TITLE "色葉おしながき"
    PERFORMER "千夜 (佐藤聡美)"
    SONGWRITER "木村有希"
    ISRC JPPI01451760
    INDEX 00 07:09:14
    INDEX 01 07:10:39
  TRACK 04 AUDIO
    TITLE "スマイルメーカー"
    PERFORMER "ココア (佐倉綾音)"
    SONGWRITER "若林充"
    ISRC JPPI01451440
    INDEX 00 11:53:24
    INDEX 01 11:54:24
  TRACK 05 AUDIO
    TITLE "Love & Gun"
    PERFORMER "リゼ (種田梨沙)"
    SONGWRITER "中村久志"
    ISRC JPPI01451450
    INDEX 00 15:39:72
    INDEX 01 15:40:72
  TRACK 06 AUDIO
    TITLE "カフェインファイター"
    PERFORMER "シャロ (内田真礼)"
    SONGWRITER "石田寛朗"
    ISRC JPPI01551980
    INDEX 00 19:22:22
    INDEX 01 19:23:22
  TRACK 07 AUDIO
    TITLE "a cup of happiness"
    PERFORMER "チノ (水瀬いのり)"
    SONGWRITER "cAnON."
    ISRC JPPI01451420
    INDEX 00 23:57:46
    INDEX 01 23:58:46
  TRACK 08 AUDIO
    TITLE "ナマイキTiny Heart"
    PERFORMER "マヤ (徳井青空)"
    SONGWRITER "ツキダタダシ"
    ISRC JPPI01551940
    INDEX 00 28:30:40
    INDEX 01 28:31:40
  TRACK 09 AUDIO
    TITLE "ナイショのはなしは夢の中で"
    PERFORMER "メグ (村川梨衣)"
    SONGWRITER "やしきん"
    ISRC JPPI01551950
    INDEX 00 32:42:27
    INDEX 01 32:42:47
  TRACK 10 AUDIO
    TITLE "ずっと一緒"
    PERFORMER "千夜 (佐藤聡美) & シャロ (内田真礼)"
    SONGWRITER "辺見さとし"
    ISRC JPPI01451750
    INDEX 00 37:40:18
    INDEX 01 37:41:18
  TRACK 11 AUDIO
    TITLE "Eを探す日常"
    PERFORMER "リゼ (種田梨沙) & シャロ (内田真礼)"
    SONGWRITER "PandaBoY"
    ISRC JPPI01451780
    INDEX 00 42:09:14
    INDEX 01 42:10:14
  TRACK 12 AUDIO
    TITLE "Rabbit Hole"
    PERFORMER "ココア (佐倉綾音) & リゼ (種田梨沙)"
    SONGWRITER "ツキダタダシ"
    ISRC JPPI01451430
    INDEX 00 46:47:08
    INDEX 01 46:48:08
  TRACK 13 AUDIO
    TITLE "きらきらエブリディ"
    PERFORMER "チマメ隊 (水瀬いのり, 徳井青空, 村川梨衣)"
    SONGWRITER "大隅知宇"
    ISRC JPPI01452690
    INDEX 00 50:31:61
    INDEX 01 50:33:61
  TRACK 14 AUDIO
    TITLE "ぽっぴんジャンプ♪"
    PERFORMER "チマメ隊 (水瀬いのり, 徳井青空, 村川梨衣)"
    SONGWRITER "木村有希"
    ISRC JPPI01450400
    INDEX 00 54:18:31
    INDEX 01 54:19:31
"#);
    }
}