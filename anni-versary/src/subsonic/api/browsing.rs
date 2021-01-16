use serde::Serialize;
use crate::subsonic::api::{Response, ResponseNotImplemented, api_not_implemented};
use crate::subsonic::api::response::{SonicArtist, SonicChild, DateTime, SonicAverageRating, SonicUserRating, SonicAlbumID3};

/// ## getMusicFolders
/// Returns all configured top-level music folders. Takes no extra parameters.
///
/// Returns a `<subsonic-response>` element with a nested `<musicFolders>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/musicFolders_example_1.xml
#[get("/getMusicFolders")]
pub fn get_music_folders() -> Response {
    unimplemented!()
}

/// ## getIndexes
/// Returns an indexed structure of all artists.
///
/// Parameter           Required	Default	Comment
/// `musicFolderId`	    No		    If specified, only return artists in the music folder with the given ID. See `getMusicFolders`.
/// `ifModifiedSince`	No		    If specified, only return a result if the artist collection has changed since the given time (in milliseconds since 1 Jan 1970).
///
/// Returns a `<subsonic-response>` element with a nested `<indexes>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/indexes_example_1.xml
#[get("/getIndexes")]
pub fn get_indexes() -> Response {
    unimplemented!()
}

/// ## getMusicDirectory
/// Returns a listing of all files in a music directory. Typically used to get list of albums for an artist, or list of songs for an album.
///
/// Parameter	Required	Default	Comment
/// `id`	        Yes         A string which uniquely identifies the music folder. Obtained by calls to getIndexes or getMusicDirectory.
///
/// Returns a `<subsonic-response>` element with a nested <directory> element on success. [Example 1]. [Example 2].
/// [Example 1]: http://www.subsonic.org/pages/inc/api/examples/directory_example_1.xml
/// [Example 2]: http://www.subsonic.org/pages/inc/api/examples/directory_example_2.xml
#[get("/getMusicDirectory")]
pub fn get_music_directory() -> Response {
    unimplemented!()
}

/// ## getGenres
/// Returns all genres.
///
/// Returns a `<subsonic-response>` element with a nested `<genres>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/genres_example_1.xml
#[get("/getGenres")]
pub fn get_genres() -> Response {
    unimplemented!()
}

/// ## getArtists
/// Similar to `getIndexes`, but organizes music according to ID3 tags.
///
/// Parameter	    Required	Default	Comment
/// `musicFolderId`	No		    If specified, only return artists in the music folder with the given ID. See `getMusicFolders`.
///
/// Returns a `<subsonic-response>` element with a nested `<artists>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/artists_example_1.xml
#[get("/getArtists")]
pub fn get_artists() -> Response {
    unimplemented!()
}

/// ## getArtist
/// Returns details for an artist, including a list of albums. This method organizes music according to ID3 tags.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The artist ID.
///
/// Returns a `<subsonic-response>` element with a nested `<artist>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/artist_example_1.xml
#[get("/getArtist")]
pub fn get_artist() -> Response {
    unimplemented!()
}

/// ## getAlbum
/// Returns details for an album, including a list of songs. This method organizes music according to ID3 tags.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The album ID.
///
/// Returns a `<subsonic-response>` element with a nested `<album>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/album_example_1.xml
#[get("/getAlbum")]
pub fn get_album() -> Response {
    unimplemented!()
}

/// ## getSong
/// Returns details for a song.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The song ID.
///
/// Returns a `<subsonic-response>` element with a nested `<song>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/song_example_1.xml
#[get("/getSong")]
pub fn get_song() -> Response {
    unimplemented!()
}

/// ## getVideos
/// Returns all video files.
///
/// Returns a `<subsonic-response>` element with a nested `<videos>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/videos_example_1.xml
#[get("/getVideos")]
pub fn get_videos() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getVideoInfo
/// Returns details for a video, including information about available audio tracks, subtitles (captions) and conversions.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The video ID.
///
/// Returns a `<subsonic-response>` element with a nested `<videoInfo>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/videoInfo_example_1.xml
#[get("/getVideoInfo")]
pub fn get_video_info() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getArtistInfo
/// Returns artist info with biography, image URLs and similar artists, using data from last.fm.
///
/// Parameter	        Required	Default	    Comment
/// `id`                Yes		                The artist, album or song ID.
/// `count`	            No	        20	        Max number of similar artists to return.
/// `includeNotPresent`	No	        false	    Whether to return artists that are not present in the media library.
///
/// Returns a `<subsonic-response>` element with a nested `<artistInfo>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/artistInfo_example_1.xml
#[get("/getArtistInfo")]
pub fn get_artist_info() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getArtistInfo2
/// Similar to `getArtistInfo`, but organizes music according to ID3 tags.
///
/// Parameter	        Required	Default	    Comment
/// `id`                Yes		                The artist ID.
/// `count`	            No	        20	        Max number of similar artists to return.
/// `includeNotPresent`	No	        false	    Whether to return artists that are not present in the media library.
///
/// Returns a `<subsonic-response>` element with a nested `<artistInfo2>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/artistInfo2_example_1.xml
#[get("/getArtistInfo2")]
pub fn get_artist_info2() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getAlbumInfo
/// Returns album notes, image URLs etc, using data from last.fm.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The album or song ID.
///
/// Returns a `<subsonic-response>` element with a nested `<albumInfo>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/albumInfo_example_1.xml
#[get("/getAlbumInfo")]
pub fn get_album_info() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getAlbumInfo2
/// Similar to `getAlbumInfo`, but organizes music according to ID3 tags.
///
/// Parameter	Required	Default	    Comment
/// id	        Yes		                The album ID.
///
/// Returns a `<subsonic-response>` element with a nested `<albumInfo>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/albumInfo_example_1.xml
#[get("/getAlbumInfo2")]
pub fn get_album_info2() -> Response {
    unimplemented!()
}

/// ## getSimilarSongs
/// Returns a random collection of songs from the given artist and similar artists, using data from last.fm. Typically used for artist radio features.
///
/// Parameter	Required	Default	    Comment
/// `id`        Yes		                The artist, album or song ID.
/// `count`	    No	        50	        Max number of songs to return.
///
/// Returns a `<subsonic-response>` element with a nested `<similarSongs>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/similarSongs_example_1.xml
#[get("/getSimilarSongs")]
pub fn get_similiar_songs() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getSimilarSongs2
/// Similar to `getSimilarSongs`, but organizes music according to ID3 tags.
///
/// Parameter	Required	Default	    Comment
/// `id`	    Yes		                The artist ID.
/// `count`	    No	        50	        Max number of songs to return.
///
/// Returns a `<subsonic-response>` element with a nested `<similarSongs2>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/similarSongs2_example_1.xml
#[get("/getSimilarSongs2")]
pub fn get_similiar_songs2() -> ResponseNotImplemented {
    api_not_implemented()
}

/// ## getTopSongs
/// Returns top songs for the given artist, using data from last.fm.
///
/// Parameter	Required	Default	    Comment
/// `artist`	Yes		    The         artist name.
/// `count`	    No	        50	        Max number of songs to return.
///
/// Returns a `<subsonic-response>` element with a nested `<topSongs>` element on success. [Example].
/// [Example]: http://www.subsonic.org/pages/inc/api/examples/topSongs_example_1.xml
#[get("/getTopSongs")]
pub fn get_top_songs() -> ResponseNotImplemented {
    api_not_implemented()
}

/// <xs:complexType name="MusicFolders">
///   <xs:sequence>
///     <xs:element name="musicFolder" type="sub:MusicFolder" minOccurs="0" maxOccurs="unbounded"/>
///   </xs:sequence>
/// </xs:complexType>
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicMusicFolder {
    /// <xs:attribute name="id" type="xs:int" use="required"/>
    id: i32,
    /// <xs:attribute name="name" type="xs:string" use="optional"/>
    name: String,
}

/// <xs:complexType name="Indexes">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicIndexes {
    /// <xs:attribute name="lastModified" type="xs:long" use="required"/>
    last_modified: u32,
    /// <xs:attribute name="ignoredArticles" type="xs:string" use="required"/>
    ignore_articles: String,
    /// <xs:sequence>
    items: Vec<IndexesItem>,
}

#[derive(Serialize, PartialEq)]
pub(crate) enum IndexesItem {
    /// <xs:element name="shortcut" type="sub:Artist" minOccurs="0" maxOccurs="unbounded"/>
    Shortcut(SonicArtist),
    /// <xs:element name="index" type="sub:Index" minOccurs="0" maxOccurs="unbounded"/>
    Index(SonicIndex),
    /// <xs:element name="child" type="sub:Child" minOccurs="0" maxOccurs="unbounded"/>
    Child(SonicChild),
}

/// <xs:complexType name="Index">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicIndex {
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:sequence>
    ///   <xs:element name="artist" type="sub:Artist" minOccurs="0" maxOccurs="unbounded"/>
    /// </xs:sequence>
    items: Vec<SonicArtist>,
}

/// <xs:complexType name="Directory">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicDirectory {
    /// <xs:attribute name="id" type="xs:string" use="required"/>
    id: String,
    /// <xs:attribute name="parent" type="xs:string" use="optional"/>
    parent: String,
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:attribute name="starred" type="xs:dateTime" use="optional"/>
    starred: DateTime,
    /// <xs:attribute name="userRating" type="sub:UserRating" use="optional"/>
    user_rating: SonicUserRating,
    /// <xs:attribute name="averageRating" type="sub:AverageRating" use="optional"/>
    average_rating: SonicAverageRating,
    /// <xs:attribute name="playCount" type="xs:long" use="optional"/>
    play_count: i64,
    /// <xs:sequence>
    /// <xs:element name="child" type="sub:Child" minOccurs="0" maxOccurs="unbounded"/>
    /// </xs:sequence>
    children: Vec<SonicChild>,
}

/// <xs:complexType name="Genres">
pub(crate) type SonicGenres = Vec<SonicGenre>;

/// <xs:complexType name="Genre" mixed="true">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicGenre {
    /// <xs:attribute name="songCount" type="xs:int" use="required"/>
    song_count: i32,
    /// <xs:attribute name="albumCount" type="xs:int" use="required"/>
    album_count: i32,
}

/// <xs:complexType name="ArtistsID3">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicArtistsID3 {
    /// <xs:attribute name="ignoredArticles" type="xs:string" use="required"/>
    ignored_articles: String,
    /// <xs:sequence>
    indexes: Vec<SonicIndexID3>,
}

/// <xs:complexType name="IndexID3">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicIndexID3 {
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:sequence>
    ///   <xs:element name="artist" type="sub:ArtistID3" minOccurs="0" maxOccurs="unbounded"/>
    /// </xs:sequence>
    artists: Vec<SonicArtistWithAlbumsID3>,
}


/// <xs:complexType name="ArtistWithAlbumsID3">
#[derive(Serialize, PartialEq)]
pub(crate) struct SonicArtistWithAlbumsID3 {
    /// <xs:attribute name="id" type="xs:string" use="required"/>
    id: String,
    /// <xs:attribute name="name" type="xs:string" use="required"/>
    name: String,
    /// <xs:attribute name="coverArt" type="xs:string" use="optional"/>
    cover_art: String,
    /// <xs:attribute name="albumCount" type="xs:int" use="required"/>
    album_count: i32,
    /// <xs:attribute name="starred" type="xs:dateTime" use="optional"/>
    starred: DateTime,
    /// <xs:complexType name="ArtistWithAlbumsID3">
    albums: Vec<SonicAlbumID3>,
}