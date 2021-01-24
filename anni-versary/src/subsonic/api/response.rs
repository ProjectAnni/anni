use serde::Serialize;
use crate::subsonic::api::system::SonicLicense;
use crate::subsonic::api::error::SonicError;
use crate::subsonic::api::browsing::{SonicMusicFolder, SonicIndexes, SonicDirectory, SonicGenres, SonicArtistsID3, SonicArtistWithAlbumsID3};

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase", rename = "subsonic-response")]
struct Response {
    // ok or failed
    status: &'static str,
    version: String,

    music_folders: Option<SonicMusicFolder>,
    indexes: Option<Vec<SonicIndexes>>,
    genres: Option<SonicGenres>,
    artists: Option<SonicArtistsID3>,
    artist: Option<SonicArtistWithAlbumsID3>,
    album: Option<SonicAlbumID3>,
    song: Option<SonicChild>,
    directory: Option<SonicDirectory>,
    license: Option<SonicLicense>,
    error: Option<SonicError>,
}

/// <xs:complexType name="Artist">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicArtist {
    /// <xs:attribute name="id" type="xs:string" use="required"/>
    id: String,
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:attribute name="starred" type="xs:dateTime" use="optional"/>
    starred: bool,
    /// <xs:attribute name="userRating" type="sub:UserRating" use="optional"/>
    user_rating: SonicUserRating,
    /// <xs:attribute name="averageRating" type="sub:AverageRating" use="optional"/>
    average_rating: SonicAverageRating,
}

/// <xs:complexType name="ArtistWithAlbumsID3">
///
/// http://www.subsonic.org/pages/inc/api/examples/artist_example_1.xml
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicAlbumID3 {
    /// <xs:attribute name="id" type="xs:string" use="required"/>
    id: String,
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:attribute name="artist" type="xs:string" use="optional"/>
    artist: String,
    /// <xs:attribute name="artistId" type="xs:string" use="optional"/>
    artist_id: String,
    /// <xs:attribute name="coverArt" type="xs:string" use="optional"/>
    cover_art: String,
    /// <xs:attribute name="songCount" type="xs:int" use="required"/>
    song_count: i32,
    /// <xs:attribute name="duration" type="xs:int" use="required"/>
    duration: i32,
    /// <xs:attribute name="playCount" type="xs:long" use="optional"/>
    play_count: i64,
    /// <xs:attribute name="created" type="xs:dateTime" use="required"/>
    created: DateTime,
    /// <xs:attribute name="starred" type="xs:dateTime" use="optional"/>
    starred: DateTime,
    /// <xs:attribute name="year" type="xs:int" use="optional"/>
    year: i32,
    /// <xs:attribute name="genre" type="xs:string" use="optional"/>
    genre: String,
    /// <xs:complexType name="AlbumWithSongsID3">
    /// http://www.subsonic.org/pages/inc/api/examples/album_example_1.xml
    songs: Vec<SonicChild>,
}

/// <xs:complexType name="Child">
///
/// http://www.subsonic.org/pages/inc/api/examples/song_example_1.xml
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicChild {
    /// <xs:attribute name="id" type="xs:string" use="required"/>
    id: String,
    /// <xs:attribute name="parent" type="xs:string" use="optional"/>
    parent: String,
    /// <xs:attribute name="isDir" type="xs:boolean" use="required"/>
    is_dir: bool,
    /// <xs:attribute name="title" type="xs:string" use="required"/>
    title: String,
    /// <xs:attribute name="album" type="xs:string" use="optional"/>
    album: String,
    /// <xs:attribute name="artist" type="xs:string" use="optional"/>
    artist: String,
    /// <xs:attribute name="track" type="xs:int" use="optional"/>
    track: i32,
    /// <xs:attribute name="year" type="xs:int" use="optional"/>
    year: i32,
    /// <xs:attribute name="genre" type="xs:string" use="optional"/>
    genre: String,
    /// <xs:attribute name="coverArt" type="xs:string" use="optional"/>
    cover_art: String,
    /// <xs:attribute name="size" type="xs:long" use="optional"/>
    size: i64,
    /// <xs:attribute name="contentType" type="xs:string" use="optional"/>
    content_type: String,
    /// <xs:attribute name="suffix" type="xs:string" use="optional"/>
    suffix: String,
    /// <xs:attribute name="transcodedContentType" type="xs:string" use="optional"/>
    transcoded_content_type: String,
    /// <xs:attribute name="transcodedSuffix" type="xs:string" use="optional"/>
    transcoded_suffix: String,
    /// <xs:attribute name="duration" type="xs:int" use="optional"/>
    duration: i32,
    /// <xs:attribute name="bitRate" type="xs:int" use="optional"/>
    bit_rate: i32,
    /// <xs:attribute name="path" type="xs:string" use="optional"/>
    path: String,
    /// <xs:attribute name="isVideo" type="xs:boolean" use="optional"/>
    is_video: bool,
    /// <xs:attribute name="userRating" type="sub:UserRating" use="optional"/>
    user_rating: SonicUserRating,
    /// <xs:attribute name="averageRating" type="sub:AverageRating" use="optional"/>
    average_rating: SonicAverageRating,
    /// <xs:attribute name="playCount" type="xs:long" use="optional"/>
    play_count: i64,
    /// <xs:attribute name="discNumber" type="xs:int" use="optional"/>
    disc_number: i32,
    /// <xs:attribute name="created" type="xs:dateTime" use="optional"/>
    created: DateTime,
    /// <xs:attribute name="starred" type="xs:dateTime" use="optional"/>
    starred: DateTime,
    /// <xs:attribute name="albumId" type="xs:string" use="optional"/>
    album_id: String,
    /// <xs:attribute name="artistId" type="xs:string" use="optional"/>
    artist_id: String,
    /// <xs:attribute name="type" type="sub:MediaType" use="optional"/>
    media_type: SonicMediaType,
    /// <xs:attribute name="bookmarkPosition" type="xs:long" use="optional"/>
    bookmark_position: i64,
    /// <xs:attribute name="originalWidth" type="xs:int" use="optional"/>
    original_width: i32,
    /// <xs:attribute name="originalHeight" type="xs:int" use="optional"/>
    original_height: i32,
}

pub(crate) type DateTime = String;
pub(crate) type SonicUserRating = u8;
pub(crate) type SonicAverageRating = f32;
pub(crate) type SonicMediaType = String; // FIXME

#[cfg(test)]
mod tests {
    use crate::subsonic::api::response::{Response, SonicError};
    use crate::subsonic::api::error::ErrorKind;

    #[test]
    fn serialize() {
        let resp = Response {
            status: "ok",
            version: "0.0.1-SNAPSHOT".to_owned(),
            music_folders: None,
            indexes: None,
            genres: None,
            artists: None,
            artist: None,
            album: None,
            song: None,
            directory: None,
            license: None,
            error: Some(SonicError::new(ErrorKind::Generic, "???")),
        };
        let data = quick_xml::se::to_string(&resp).unwrap();
        let _r = data;
    }
}