use sqlx::{migrate::MigrateDatabase, sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

use crate::server::{config::Config, crypto::Crypto};

pub mod storage;

pub async fn create_pool(cfg: &Config) -> sqlx::Result<SqlitePool> {
    let db_path = &cfg.storage.database_path;

    if !sqlx::Sqlite::database_exists(db_path)
        .await
        .unwrap_or(false)
    {
        sqlx::Sqlite::create_database(db_path).await?;
    }

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{db_path}"))?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub udid: String,
    pub ip: String,
    pub port: i64,
    pub pairing_blob: Vec<u8>,
    pub last_seen: Option<i64>,
    pub discovery: String,
    pub mdns_ip: Option<String>,
    pub mdns_seen_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountRow {
    pub id: String,
    pub apple_id: String,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppRow {
    pub id: String,
    pub device_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub ipa_path: Option<String>,
    pub account_id: Option<String>,
    pub installed_at: Option<i64>,
    pub last_refreshed: Option<i64>,
    pub refresh_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobRow {
    pub id: String,
    pub app_id: String,
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub progress: i64,
    pub stage: Option<String>,
}

pub async fn list_devices(pool: &SqlitePool) -> sqlx::Result<Vec<DeviceRow>> {
    sqlx::query_as!(
        DeviceRow,
        r#"SELECT id, name, udid, ip, port, pairing_blob, last_seen, discovery, mdns_ip, mdns_seen_at
           FROM devices ORDER BY name"#
    )
    .fetch_all(pool)
    .await
}

pub async fn get_device(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<DeviceRow>> {
    sqlx::query_as!(
        DeviceRow,
        r#"SELECT id, name, udid, ip, port, pairing_blob, last_seen, discovery, mdns_ip, mdns_seen_at
           FROM devices WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_device_mdns_ip(pool: &SqlitePool, id: &str, ip: &str) -> sqlx::Result<()> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query!(
        "UPDATE devices SET mdns_ip = ?, mdns_seen_at = ? WHERE id = ?",
        ip,
        now,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_device(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    udid: &str,
    ip: &str,
    port: i64,
    pairing_blob: &[u8],
    discovery: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"INSERT INTO devices (id, name, udid, ip, port, pairing_blob, discovery)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id,
        name,
        udid,
        ip,
        port,
        pairing_blob,
        discovery
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_device_ip(pool: &SqlitePool, id: &str, ip: &str) -> sqlx::Result<()> {
    sqlx::query!("UPDATE devices SET ip = ? WHERE id = ?", ip, id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_device_pairing(pool: &SqlitePool, id: &str, blob: &[u8]) -> sqlx::Result<()> {
    sqlx::query!("UPDATE devices SET pairing_blob = ? WHERE id = ?", blob, id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_device(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM devices WHERE id = ?", id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_accounts(pool: &SqlitePool) -> sqlx::Result<Vec<AccountRow>> {
    sqlx::query_as!(
        AccountRow,
        r#"SELECT id, apple_id, team_id, team_name FROM apple_accounts ORDER BY apple_id"#
    )
    .fetch_all(pool)
    .await
}

pub async fn get_account_session(
    pool: &SqlitePool,
    id: &str,
    crypto: &Crypto,
) -> sqlx::Result<Option<Vec<u8>>> {
    let row = sqlx::query!("SELECT session_blob FROM apple_accounts WHERE id = ?", id)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|r| crypto.decrypt(&r.session_blob).ok()))
}

pub async fn insert_account(
    pool: &SqlitePool,
    id: &str,
    apple_id: &str,
    session_blob: &[u8],
    team_id: Option<&str>,
    team_name: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"INSERT INTO apple_accounts (id, apple_id, session_blob, team_id, team_name)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT(apple_id) DO UPDATE SET
             session_blob = excluded.session_blob,
             team_id = excluded.team_id,
             team_name = excluded.team_name"#,
        id,
        apple_id,
        session_blob,
        team_id,
        team_name
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_account(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM apple_accounts WHERE id = ?", id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_apps_for_device(pool: &SqlitePool, device_id: &str) -> sqlx::Result<Vec<AppRow>> {
    sqlx::query_as!(
        AppRow,
        r#"SELECT id, device_id, bundle_id, display_name, version, ipa_path,
                  account_id, installed_at, last_refreshed,
                  refresh_enabled as "refresh_enabled: bool"
           FROM apps WHERE device_id = ? ORDER BY display_name"#,
        device_id
    )
    .fetch_all(pool)
    .await
}

pub async fn get_app(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<AppRow>> {
    sqlx::query_as!(
        AppRow,
        r#"SELECT id, device_id, bundle_id, display_name, version, ipa_path,
                  account_id, installed_at, last_refreshed,
                  refresh_enabled as "refresh_enabled: bool"
           FROM apps WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await
}

/// Upsert an app record and return the actual row `id` (which may differ from `id`
/// when a conflict occurs and the pre-existing row's id is kept).
/// installed_at isn't updated until confirmed install
#[allow(clippy::too_many_arguments)]
pub async fn upsert_app(
    pool: &SqlitePool,
    id: &str,
    device_id: &str,
    bundle_id: &str,
    display_name: &str,
    version: Option<&str>,
    ipa_path: Option<&str>,
    account_id: Option<&str>,
) -> sqlx::Result<String> {
    let actual_id = sqlx::query_scalar!(
        r#"INSERT INTO apps (id, device_id, bundle_id, display_name, version, ipa_path, account_id)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(device_id, bundle_id) DO UPDATE SET
             display_name = excluded.display_name,
             version = excluded.version,
             ipa_path = COALESCE(excluded.ipa_path, apps.ipa_path),
             account_id = COALESCE(excluded.account_id, apps.account_id)
           RETURNING id"#,
        id,
        device_id,
        bundle_id,
        display_name,
        version,
        ipa_path,
        account_id
    )
    .fetch_one(pool)
    .await?;
    Ok(actual_id)
}

/// Mark an app as successfully installed, rewriting `bundle_id` to the value
/// the device actually has (isideload appends the team ID to the original).
///
/// If another row on the same device already has the real bundle ID (e.g. a
/// prior install), the two rows are merged into that existing row and our row
/// is removed. Returns the surviving row's id. That way people can update.
pub async fn mark_app_installed(
    pool: &SqlitePool,
    app_id: &str,
    real_bundle_id: &str,
) -> sqlx::Result<String> {
    let now = chrono::Utc::now().timestamp();

    let row = sqlx::query!(
        "SELECT device_id, bundle_id, display_name, version, ipa_path, account_id
           FROM apps WHERE id = ?",
        app_id
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(app_id.to_string());
    };

    if row.bundle_id == real_bundle_id {
        sqlx::query!(
            "UPDATE apps SET installed_at = COALESCE(installed_at, ?), last_refreshed = ?
               WHERE id = ?",
            now,
            now,
            app_id
        )
        .execute(pool)
        .await?;
        return Ok(app_id.to_string());
    }

    let conflict: Option<String> = sqlx::query_scalar!(
        "SELECT id FROM apps WHERE device_id = ? AND bundle_id = ? AND id != ?",
        row.device_id,
        real_bundle_id,
        app_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(target) = conflict {
        sqlx::query!(
            "UPDATE apps SET
               display_name = ?,
               version      = COALESCE(?, version),
               ipa_path     = COALESCE(?, ipa_path),
               account_id   = COALESCE(?, account_id),
               installed_at = ?,
               last_refreshed = ?
             WHERE id = ?",
            row.display_name,
            row.version,
            row.ipa_path,
            row.account_id,
            now,
            now,
            target
        )
        .execute(pool)
        .await?;
        sqlx::query!(
            "UPDATE jobs SET app_id = ? WHERE app_id = ?",
            target,
            app_id
        )
        .execute(pool)
        .await?;
        sqlx::query!("DELETE FROM apps WHERE id = ?", app_id)
            .execute(pool)
            .await?;
        Ok(target)
    } else {
        sqlx::query!(
            "UPDATE apps SET bundle_id = ?, installed_at = COALESCE(installed_at, ?),
               last_refreshed = ? WHERE id = ?",
            real_bundle_id,
            now,
            now,
            app_id
        )
        .execute(pool)
        .await?;
        Ok(app_id.to_string())
    }
}

pub async fn delete_app(pool: &SqlitePool, app_id: &str) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM apps WHERE id = ?", app_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_app_refresh(pool: &SqlitePool, app_id: &str, enabled: bool) -> sqlx::Result<()> {
    let v = enabled as i64;
    sqlx::query!(
        "UPDATE apps SET refresh_enabled = ? WHERE id = ?",
        v,
        app_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobWithContext {
    pub id: String,
    pub app_id: String,
    pub app_name: String,
    pub device_id: String,
    pub device_name: String,
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub progress: i64,
    pub stage: Option<String>,
}

/// Lists jobs for the queue view, most-recently-created first, with active
/// (running, then queued) jobs surfaced ahead of finished ones.
pub async fn list_jobs(pool: &SqlitePool, limit: i64) -> sqlx::Result<Vec<JobWithContext>> {
    let rows = sqlx::query(
        "SELECT j.id, j.app_id, a.display_name as app_name, a.device_id as device_id, \
                d.name as device_name, j.kind, j.status, j.error, j.started_at, \
                j.finished_at, j.progress, j.stage \
         FROM jobs j \
         JOIN apps a ON a.id = j.app_id \
         JOIN devices d ON d.id = a.device_id \
         ORDER BY CASE j.status WHEN 'running' THEN 0 WHEN 'queued' THEN 1 ELSE 2 END, \
                  j.rowid DESC \
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            use sqlx::Row;
            JobWithContext {
                id: r.get("id"),
                app_id: r.get("app_id"),
                app_name: r.get("app_name"),
                device_id: r.get("device_id"),
                device_name: r.get("device_name"),
                kind: r.get("kind"),
                status: r.get("status"),
                error: r.get("error"),
                started_at: r.get("started_at"),
                finished_at: r.get("finished_at"),
                progress: r.get("progress"),
                stage: r.get("stage"),
            }
        })
        .collect())
}

pub async fn get_job(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<JobRow>> {
    let row = sqlx::query(
        "SELECT id, app_id, kind, status, error, started_at, finished_at, progress, stage \
         FROM jobs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        use sqlx::Row;
        JobRow {
            id: r.get("id"),
            app_id: r.get("app_id"),
            kind: r.get("kind"),
            status: r.get("status"),
            error: r.get("error"),
            started_at: r.get("started_at"),
            finished_at: r.get("finished_at"),
            progress: r.get("progress"),
            stage: r.get("stage"),
        }
    }))
}

pub async fn update_job_progress(
    pool: &SqlitePool,
    id: &str,
    progress: i64,
    stage: &str,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE jobs SET progress = ?, stage = ? WHERE id = ?")
        .bind(progress)
        .bind(stage)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_job(pool: &SqlitePool, id: &str, app_id: &str, kind: &str) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO jobs (id, app_id, kind, status) VALUES (?, ?, ?, 'queued')",
        id,
        app_id,
        kind
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_job_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> sqlx::Result<()> {
    let now = chrono::Utc::now().timestamp();
    if status == "running" {
        sqlx::query!(
            "UPDATE jobs SET status = ?, started_at = ? WHERE id = ?",
            status,
            now,
            id
        )
        .execute(pool)
        .await?;
    } else {
        sqlx::query!(
            "UPDATE jobs SET status = ?, error = ?, finished_at = ? WHERE id = ?",
            status,
            error,
            now,
            id
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn apps_needing_refresh(
    pool: &SqlitePool,
    window_days: i64,
) -> sqlx::Result<Vec<AppRow>> {
    let cert_lifetime_secs: i64 = 7 * 86400;
    let threshold = chrono::Utc::now().timestamp() - (cert_lifetime_secs - window_days * 86400);
    sqlx::query_as!(
        AppRow,
        r#"SELECT id, device_id, bundle_id, display_name, version, ipa_path,
                  account_id, installed_at, last_refreshed,
                  refresh_enabled as "refresh_enabled: bool"
           FROM apps
           WHERE refresh_enabled = 1
             AND ipa_path IS NOT NULL
             AND account_id IS NOT NULL
             AND installed_at IS NOT NULL
             AND COALESCE(last_refreshed, installed_at) < ?"#,
        threshold
    )
    .fetch_all(pool)
    .await
}
