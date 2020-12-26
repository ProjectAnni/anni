mod auth;

use actix_web::{HttpServer, Responder, HttpResponse, App, middleware};
use r2d2_sqlite::SqliteConnectionManager;
use r2d2::Pool;
use actix_web::dev::Service;
use actix_web::http::header::CONTENT_TYPE;
use actix_web::http::HeaderValue;

#[actix_web::get("/ping")]
async fn ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

#[actix_web::main]
pub async fn anni_versary() -> std::io::Result<()> {
    let manager = SqliteConnectionManager::file("anni.db");
    let pool = Pool::new(manager).unwrap();
    HttpServer::new(move ||
        App::new()
            .data(pool.clone())
            .wrap(middleware::Logger::default())
            .service(ping)
            .wrap_fn(|req, srv| {
                println!("Requested: {} ", req.path());
                let fut = srv.call(req);
                async {
                    let mut res = fut.await?;
                    res.headers_mut().insert(
                        CONTENT_TYPE, HeaderValue::from_static("text/plain"),
                    );
                    Ok(res)
                }
            })
    )
        .bind("127.0.0.1:8080")?
        .run()
        .await
}