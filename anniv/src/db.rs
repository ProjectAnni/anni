use sqlx::{Postgres, Pool};

pub(crate) async fn init_db(pool: Pool<Postgres>) -> anyhow::Result<()> {
    // Types
    sqlx::query(r#"
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'anni_song') THEN
      -- Song type
      CREATE TYPE anni_song AS (
        catalog     TEXT,
        track_id    INTEGER
      );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'anni_playlist_song') THEN
      -- Song type used in playlist
      CREATE TYPE anni_playlist_song AS (
        catalog        TEXT,
        track_id       INTEGER,
        description    TEXT
      );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'anni_lyric_type') THEN
      -- Lyric type
      CREATE TYPE anni_lyric_type AS ENUM('lrc', 'text');
    END IF;
END
$$;"#).execute(&pool).await?;

    // User
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

    // Playlist
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_playlist (
  id             UUID,                          -- ID of playlist.
  owner_id       UUID,                          -- Owner of playlist.
  is_public      BOOLEAN,                       -- Public playlist or not.
  description    TEXT,                          -- Playlist description text.
  songs          anni_playlist_song[] NOT NULL, -- Songs in playlist.
  PRIMARY KEY (id, owner_id)
);"#).execute(&pool).await?;

    // Lyric
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_lyric (
  id              UUID,               -- ID of lyric
  song            anni_song NOT NULL, -- Song of lyric
  lyric_type      anni_lyric_type,    -- Lyric type
  lyric_locale    TEXT,               -- Language
  lyric_text      TEXT,               -- Lyric content
  PRIMARY KEY (id, song)
);"#).execute(&pool).await?;

    // Play history
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_play_history (
  owner_id       UUID,                                    -- Played user.
  song           anni_song,                               -- Played song.
  play_time      TIMESTAMP,                               -- Played time.
  play_client    TEXT,                                    -- Played client.
  play_count     INTEGER CHECK (play_count > 0) NOT NULL, -- Play count of a song.
  PRIMARY KEY (owner_id, song, play_time)
);"#).execute(&pool).await?;

    // Share
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_share (
  id             UUID PRIMARY KEY,
  owner_id       UUID NOT NULL,
  song           anni_song,
  playlist_owner UUID,                                                                       -- UUID of playlist owner.
  playlist_id    UUID,                                                                       -- UUID of playlist to share.
  start_time     TIMESTAMP NOT NULL,                                                              -- Share start time.
  end_time       TIMESTAMP,                                                                       -- Share end time, NULL to be infinity.
  CHECK (((playlist_owner IS NOT NULL) AND (playlist_id IS NOT NULL)) <> (song IS NOT NULL)) -- Playlist is NOT NULL ^(xor) song is NOT NULL.
                                                                                             -- Only one of them can be NOT NULL.
);"#).execute(&pool).await?;

    // Options
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS anni_options (
  option_name      TEXT PRIMARY KEY,
  option_value     TEXT,
  default_value    TEXT NOT NULL
);"#).execute(&pool).await?;

    Ok(())
}