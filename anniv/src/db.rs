use sqlx::{Postgres, Pool};

pub(crate) async fn init_db(pool: Pool<Postgres>) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_user (
  id              UUID PRIMARY KEY,     -- User id.
  inviter_id      UUID NOT NULL,        -- The first user is invited by 'anni',
                                        --   which means the inviter is '5e9d2c21-963f-52c3-b832-fd4d3adc96cd',
                                        --   equals to uuidv5(domain, 'anni.mmf.moe').
  display_name    TEXT NOT NULL,        -- Display name of a user.
  username        TEXT NOT NULL UNIQUE, -- User identifier string.
  email           TEXT NOT NULL,        -- Login email.
  password        TEXT NOT NULL,        -- Hash of user password.
  avatar          TEXT                  -- Link to user avatar.
);"#).execute(&pool).await?;
    Ok(())
}