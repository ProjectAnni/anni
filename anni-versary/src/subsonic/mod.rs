use rocket::Route;

mod api;

pub const PATH: &'static str = "/rest";

pub fn routes() -> Vec<Route> {
    routes![
        api::system::ping, api::system::get_license,

        api::browsing::get_music_folders, api::browsing::get_indexes,
        api::browsing::get_music_directory, api::browsing::get_genres,
        api::browsing::get_artists, api::browsing::get_artist,
        api::browsing::get_album, api::browsing::get_song,
        api::browsing::get_videos, api::browsing::get_video_info,
        api::browsing::get_artist_info, api::browsing::get_artist_info2,
        api::browsing::get_album_info, api::browsing::get_album_info2,
        api::browsing::get_similiar_songs, api::browsing::get_similiar_songs2,
        api::browsing::get_top_songs
    ]
}
