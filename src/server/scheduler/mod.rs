use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::server::{
    db,
    state::{AppState, JobKind, JobRequest},
};

pub async fn run_scheduler(state: AppState) {
    let interval_secs = state.config.scheduler.interval_hours * 3600;
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

    info!(
        "Refresh scheduler started, checking every {} hours",
        state.config.scheduler.interval_hours
    );

    loop {
        ticker.tick().await;
        check_and_enqueue(&state).await;
    }
}

async fn check_and_enqueue(state: &AppState) {
    let window = state.config.scheduler.refresh_window_days;
    let apps = match db::apps_needing_refresh(&state.db, window).await {
        Ok(a) => a,
        Err(e) => {
            error!("Scheduler: failed to query apps: {e}");
            return;
        }
    };

    if apps.is_empty() {
        info!("Scheduler: no apps need refresh");
        return;
    }

    info!(
        "Scheduler: {} app(s) need refresh, verifying device state",
        apps.len()
    );

    // Group by device so we only open one instproxy connection per device.
    let mut apps_by_device: std::collections::HashMap<String, Vec<db::AppRow>> =
        std::collections::HashMap::new();
    for app in apps {
        apps_by_device
            .entry(app.device_id.clone())
            .or_default()
            .push(app);
    }

    let mut verified_apps = Vec::new();
    for (device_id, device_apps) in apps_by_device {
        let device = match db::get_device(&state.db, &device_id).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                warn!("Scheduler: device {device_id} not found");
                continue;
            }
            Err(e) => {
                error!("Scheduler: failed to get device {device_id}: {e}");
                continue;
            }
        };

        let pairing_bytes = match state.crypto.decrypt(&device.pairing_blob) {
            Ok(b) => b,
            Err(e) => {
                warn!("Scheduler: failed to decrypt pairing for device {device_id}: {e}");
                continue;
            }
        };

        let installed = match crate::server::sideload::list_installed_bundle_ids(
            &device.ip,
            device.mdns_ip.as_deref(),
            &pairing_bytes,
        )
        .await
        {
            Ok(ids) => ids,
            Err(e) => {
                warn!("Scheduler: device {device_id} unreachable, skipping: {e}");
                continue;
            }
        };

        if installed.is_empty() {
            warn!("Scheduler: device {device_id} returned no apps, skipping prune");
            verified_apps.extend(device_apps);
            continue;
        }

        for app in device_apps {
            if installed.contains(&app.bundle_id) {
                verified_apps.push(app);
            } else {
                info!(
                    "Scheduler: {} not found on device, removing from DB",
                    app.bundle_id
                );
                if let Err(e) = db::delete_app(&state.db, &app.id).await {
                    error!("Scheduler: failed to delete stale app {}: {e}", app.id);
                }
            }
        }
    }

    if verified_apps.is_empty() {
        info!("Scheduler: no apps verified for refresh");
        return;
    }

    info!(
        "Scheduler: {} app(s) verified for refresh",
        verified_apps.len()
    );

    for app in verified_apps {
        let account_id = match app.account_id {
            Some(ref id) => id.clone(),
            None => {
                warn!("Scheduler: no account for app {}", app.id);
                continue;
            }
        };

        let job_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = db::insert_job(&state.db, &job_id, &app.id, "refresh").await {
            error!("Scheduler: failed to insert job: {e}");
            continue;
        }

        let req = JobRequest {
            job_id: job_id.clone(),
            app_id: app.id.clone(),
            device_id: app.device_id.clone(),
            account_id,
            kind: JobKind::Refresh,
        };

        if let Err(e) = state.job_tx.send(req).await {
            error!("Scheduler: failed to enqueue job {job_id}: {e}");
        } else {
            info!(
                "Scheduler: enqueued refresh job {job_id} for app {}",
                app.id
            );
        }
    }
}

pub async fn run_workers(state: AppState, mut rx: mpsc::Receiver<JobRequest>) {
    let workers = state.config.scheduler.worker_threads;
    info!("Job worker pool started with {workers} workers");

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(workers));

    while let Some(req) = rx.recv().await {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let state = state.clone();

        tokio::spawn(async move {
            let _permit = permit;
            execute_job(state, req).await;
        });
    }
}

async fn execute_job(state: AppState, req: JobRequest) {
    let job_id = &req.job_id;
    info!(
        "Running job {job_id} ({:?})",
        match &req.kind {
            JobKind::Install { .. } => "install",
            JobKind::Refresh => "refresh",
        }
    );

    if let Err(e) = db::update_job_status(&state.db, job_id, "running", None).await {
        error!("Job {job_id}: failed to mark running: {e}");
        return;
    }

    let result = run_job(&state, &req).await;

    match result {
        Ok(real_bundle_id) => {
            let _ = db::update_job_status(&state.db, job_id, "done", None).await;
            let _ = db::mark_app_installed(&state.db, &req.app_id, &real_bundle_id).await;
            info!("Job {job_id} complete (bundle id: {real_bundle_id})");
        }
        Err(e) => {
            let msg = e.to_string();
            error!("Job {job_id} failed: {msg}");
            let _ = db::update_job_status(&state.db, job_id, "failed", Some(&msg)).await;
        }
    }
}

async fn run_job(state: &AppState, req: &JobRequest) -> anyhow::Result<String> {
    let device = db::get_device(&state.db, &req.device_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Device {} not found", req.device_id))?;

    let account_row = sqlx::query!(
        "SELECT apple_id, session_blob FROM apple_accounts WHERE id = ?",
        req.account_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Account {} not found", req.account_id))?;

    let anisette_url = state.anisette_url.read().await.clone();

    if let JobKind::Refresh = req.kind {
        let app = db::get_app(&state.db, &req.app_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("App {} not found", req.app_id))?;

        let job_id = req.job_id.clone();
        let _ = db::update_job_progress(&state.db, &job_id, 0, "refreshing").await;

        match crate::server::sideload::refresh_provisioning_profile(
            &state.db,
            &state.crypto,
            &account_row.apple_id,
            &account_row.session_blob,
            &device.ip,
            device.mdns_ip.as_deref(),
            &device.pairing_blob,
            &app.bundle_id,
            &anisette_url,
        )
        .await
        {
            Ok(()) => {
                let _ = db::update_job_progress(&state.db, &job_id, 100, "refreshing").await;
                return Ok(app.bundle_id);
            }
            Err(e) => {
                warn!("Job {job_id}: profile refresh failed, falling back to full install: {e}");
            }
        }
    }

    let ipa_path = match &req.kind {
        JobKind::Install { ipa_path } => ipa_path.clone(),
        JobKind::Refresh => {
            let app = db::get_app(&state.db, &req.app_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("App {} not found", req.app_id))?;
            app.ipa_path
                .ok_or_else(|| anyhow::anyhow!("App has no stored IPA"))?
        }
    };

    let db = state.db.clone();
    let job_id = req.job_id.clone();
    crate::server::sideload::install_ipa(
        &state.db,
        &state.crypto,
        &account_row.apple_id,
        &account_row.session_blob,
        &device.ip,
        device.mdns_ip.as_deref(),
        &device.name,
        &device.udid,
        &device.pairing_blob,
        &ipa_path,
        &anisette_url,
        move |progress, stage| {
            let db = db.clone();
            let job_id = job_id.clone();
            tokio::spawn(async move {
                let _ =
                    crate::server::db::update_job_progress(&db, &job_id, progress as i64, stage)
                        .await;
            });
        },
    )
    .await
}
