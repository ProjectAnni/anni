use sqlx::{Postgres, Pool, types::chrono};

pub(crate) async fn init_db(pool: Pool<Postgres>) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS annil_user (
  username        TEXT PRIMARY KEY,     -- Username.
  inviter         TEXT NOT NULL,        -- Inviter username.
  email           TEXT NOT NULL UNIQUE, -- Login email.
  password        TEXT NOT NULL,        -- Hash of user password.
  invoke_time     TIMESTAMPTZ NOT NULL  -- Time of last jwt revoke.
);"#).execute(&pool).await?;
    Ok(())
}

pub(crate) async fn iat_valid(pool: Pool<Postgres>, username: &str, iat: u64) -> bool {
    let row: Result<(chrono::DateTime<chrono::Utc>, ), _> = sqlx::query_as(r#"SELECT invoke_time FROM annil_user WHERE username = $1;"#)
        .bind(username)
        .fetch_one(&pool).await;
    row.is_ok() && ((row.unwrap().0.timestamp() as u64) < iat)
}