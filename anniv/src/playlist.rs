use actix_web::{web, Responder, get, patch, delete, put, HttpResponse};
use serde::Serialize;
use crate::AppState;

#[derive(sqlx::FromRow, Serialize)]
struct Playlist {
    id: String,
    description: String,
    is_public: bool,
}

#[derive(sqlx::FromRow, Serialize)]
struct PlaylistDetail {
    id: String,
    description: String,
    is_public: bool,
    songs: Vec<(String, i32, String)>,
}

#[derive(sqlx::FromRow, Serialize)]
struct PlaylistSong {
    catalog: String,
    track_id: u32,
    description: String,
}

#[get("/user/{username}/playlist")]
pub(crate) async fn playlist_list(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let username = path.into_inner();
    sqlx::query_as::<_, Playlist>(
        r#"
        SELECT id, description FROM anni_playlist
          where
            owner_id = (SELECT owner_id FROM anni_user WHERE username = $1)
          AND
            (is_public = true OR $2)"#,
    )
        .bind(username)
        .bind(username == "") // TODO: username of current user
        .fetch_all(&data.pool.clone())
        .await
        .map(|p| HttpResponse::Ok().json(p))
        .unwrap_or(HttpResponse::InternalServerError().finish())
}

#[get("/user/{username}/playlist/{playlist_id}")]
pub(crate) async fn playlist_detail(path: web::Path<(String, String)>, data: web::Data<AppState>) -> impl Responder {
    let (username, playlist_id) = path.into_inner();
    sqlx::query_as::<_, PlaylistDetail>(
        "SELECT * FROM anni_playlist where owner_id = (SELECT owner_id from anni_user WHERE username = $1) AND id = ($2::uuid)"
    )
        .bind(username)
        .bind(playlist_id)
        .fetch_optional(&data.pool.clone())
        .await
        .map(|p| HttpResponse::Ok().json(p)) // TODO: handle Option & user here
        .unwrap_or(HttpResponse::InternalServerError().finish())
}

#[put("/user/{username}/playlist")]
pub(crate) async fn playlist_new(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let username = path.into_inner();
    ""
}

#[patch("/user/{username}/playlist/{playlist_id}")]
pub(crate) async fn playlist_modify(path: web::Path<(String, String)>, data: web::Data<AppState>) -> impl Responder {
    ""
}

#[delete("/user/{username}/playlist/{playlist_id}")]
pub(crate) async fn playlist_delete(path: web::Path<(String, String)>, data: web::Data<AppState>) -> impl Responder {
    ""
}
