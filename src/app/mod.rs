pub mod components;
pub mod pages;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    path, StaticSegment,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/jas.css" />
        <Title text="JAS - Jackson's App Sensei" />

        <Router>
            <nav class="nav">
                <span class="nav-brand">"JAS"</span>
                <A href="/" attr:class="nav-link">
                    "Dashboard"
                </A>
                <A href="/devices" attr:class="nav-link">
                    "Devices"
                </A>
                <A href="/accounts" attr:class="nav-link">
                    "Accounts"
                </A>
                <A href="/queue" attr:class="nav-link">
                    "Queue"
                </A>
                <A href="/settings" attr:class="nav-link">
                    "Settings"
                </A>
            </nav>
            <main class="main-content">
                <Routes fallback=|| view! { <p class="error">"Page not found."</p> }>
                    <Route path=StaticSegment("") view=pages::dashboard::Dashboard />
                    <Route path=StaticSegment("devices") view=pages::devices::Devices />
                    <Route path=path!("/devices/:id") view=pages::device_detail::DeviceDetail />
                    <Route path=StaticSegment("accounts") view=pages::accounts::Accounts />
                    <Route path=StaticSegment("queue") view=pages::queue::Queue />
                    <Route path=StaticSegment("settings") view=pages::settings::Settings />
                </Routes>
            </main>
            <footer class="footer">"On to eternal perfection"</footer>
        </Router>
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub udid: String,
    pub ip: String,
    pub port: i64,
    pub last_seen: Option<i64>,
    pub discovery: String,
    pub mdns_ip: Option<String>,
    pub mdns_seen_at: Option<i64>,
}

/// Per-device summary used by the Dashboard
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceSummary {
    pub device: DeviceInfo,
    pub app_count: usize,
    pub expiring_soon_count: usize,
    pub expired_count: usize,
    pub next_expires_at: Option<i64>,
    pub expiring_soon_threshold_secs: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub apple_id: String,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LcCertExport {
    pub p12_b64: String,
    pub serial: String,
    pub password: String,
}

/// A trusted phone number Apple offers as an SMS destination.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrustedNumberInfo {
    pub id: u32,
    /// Masked number as Apple formats it, e.g. "+1 (•••) •••-••89".
    pub number: String,
    pub last_two_digits: String,
}

/// What Apple is currently asking the user for during two-factor auth.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TwoFactorPrompt {
    /// The last method failed and Apple won't say which one to use; the user
    /// has to pick a different one rather than enter a code.
    pub unknown: bool,
    /// True when the pending code was sent by SMS rather than to trusted devices.
    pub sms: bool,
    pub numbers: Vec<TrustedNumberInfo>,
    /// Which number the pending SMS went to, when `sms` is set.
    pub selected_number_id: Option<u32>,
}

/// The user's answer to a [`TwoFactorPrompt`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TwoFactorAction {
    SubmitCode(String),
    /// Text a fresh code to the trusted number with this id.
    SendSms(u32),
    /// Push a fresh code to the account's trusted devices.
    SendToDevices,
    /// Re-send using whichever method is already in flight.
    ResendCode,
    Abort,
}

/// Where a login attempt currently stands.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LoginStep {
    /// Still talking to Apple; poll again.
    Pending,
    /// Blocked on the user, who should be shown `prompt`.
    TwoFactor { prompt: TwoFactorPrompt },
    /// Logged in and the account has been saved.
    Complete,
}

#[cfg(feature = "ssr")]
impl From<isideload::auth::apple_account::TwoFactorCallbackParams> for TwoFactorPrompt {
    fn from(p: isideload::auth::apple_account::TwoFactorCallbackParams) -> Self {
        Self {
            unknown: p.unknown,
            sms: p.sms,
            numbers: p
                .numbers
                .into_iter()
                .map(|n| TrustedNumberInfo {
                    id: n.id,
                    number: n.number_with_dial_code,
                    last_two_digits: n.last_two_digits,
                })
                .collect(),
            selected_number_id: p.selected_number_id,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppIdEntry {
    pub name: String,
    pub identifier: String,
    /// Unix timestamp (seconds) the App ID's certificate expires at.
    pub expiration_date: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppIdsResult {
    pub entries: Vec<AppIdEntry>,
    pub max_quantity: Option<u64>,
    pub available_quantity: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub device_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub has_ipa: bool,
    pub installed_at: Option<i64>,
    pub last_refreshed: Option<i64>,
    pub refresh_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobInfo {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueEntry {
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

#[server]
pub async fn list_devices() -> Result<Vec<DeviceInfo>, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let rows = db::list_devices(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| DeviceInfo {
            id: r.id,
            name: r.name,
            udid: r.udid,
            ip: r.ip,
            port: r.port,
            last_seen: r.last_seen,
            discovery: r.discovery,
            mdns_ip: r.mdns_ip,
            mdns_seen_at: r.mdns_seen_at,
        })
        .collect())
}

#[server]
pub async fn dashboard_summary() -> Result<Vec<DeviceSummary>, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let devices = db::list_devices(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    const CERT_LIFETIME_SECS: i64 = 7 * 86400;
    let window_secs = state.config.scheduler.refresh_window_days * 86400;
    let now = chrono::Utc::now().timestamp();

    let mut out = Vec::with_capacity(devices.len());
    for d in devices {
        let apps = db::list_apps_for_device(&state.db, &d.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let mut app_count = 0usize;
        let mut expiring_soon = 0usize;
        let mut expired = 0usize;
        let mut next_expires_at: Option<i64> = None;

        for app in apps {
            let Some(installed) = app.installed_at else {
                continue;
            };
            app_count += 1;

            let last_event = app.last_refreshed.unwrap_or(installed).max(installed);
            let expires_at = last_event + CERT_LIFETIME_SECS;
            let remaining = expires_at - now;

            if remaining <= 0 {
                expired += 1;
            } else {
                if remaining <= window_secs {
                    expiring_soon += 1;
                }
                next_expires_at = Some(match next_expires_at {
                    Some(prev) => prev.min(expires_at),
                    None => expires_at,
                });
            }
        }

        out.push(DeviceSummary {
            device: DeviceInfo {
                id: d.id,
                name: d.name,
                udid: d.udid,
                ip: d.ip,
                port: d.port,
                last_seen: d.last_seen,
                discovery: d.discovery,
                mdns_ip: d.mdns_ip,
                mdns_seen_at: d.mdns_seen_at,
            },
            app_count,
            expiring_soon_count: expiring_soon,
            expired_count: expired,
            next_expires_at,
            expiring_soon_threshold_secs: window_secs,
        });
    }

    Ok(out)
}

#[server]
pub async fn register_device(
    ip: String,
    pairing_blob_b64: String,
) -> Result<String, ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use base64::Engine;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let pairing_bytes = base64::engine::general_purpose::STANDARD
        .decode(&pairing_blob_b64)
        .map_err(|e| ServerFnError::new(format!("Invalid base64: {e}")))?;

    // Connect to the device, verify the pairing file, and auto-fetch its identity.
    let (udid, name) = sideload::fetch_device_identity(&ip, &pairing_bytes)
        .await
        .map_err(|e| ServerFnError::new(format!("Device connection failed: {e}")))?;

    let encrypted = state.crypto.encrypt(&pairing_bytes);

    let id = uuid::Uuid::new_v4().to_string();
    db::insert_device(
        &state.db, &id, &name, &udid, &ip, 49152, &encrypted, "static",
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(id)
}

#[server]
pub async fn delete_device(id: String) -> Result<(), ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    db::delete_device(&state.db, &id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_accounts() -> Result<Vec<AccountInfo>, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let rows = db::list_accounts(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| AccountInfo {
            id: r.id,
            apple_id: r.apple_id,
            team_id: r.team_id,
            team_name: r.team_name,
        })
        .collect())
}

/// Begin an Apple ID login. Returns (session_key, needs_2fa).
#[server]
pub async fn begin_login(
    apple_id: String,
    password: String,
) -> Result<(String, LoginStep), ServerFnError> {
    use crate::server::sideload;
    use crate::server::state::{AppState, LoginData, PendingLogin};
    use isideload::{
        auth::apple_account::{AppleAccount, TwoFactorCallbackParams, TwoFactorCallbackResponse},
        dev::{developer_session::DeveloperSession, teams::TeamsApi},
    };
    use leptos::prelude::use_context;
    use std::sync::Arc;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let session_key = uuid::Uuid::new_v4().to_string();

    // Carries the user's chosen action to the login thread blocked in the 2FA callback.
    let (action_tx, action_rx) = std::sync::mpsc::sync_channel::<TwoFactorAction>(1);
    let result_cell: Arc<tokio::sync::Mutex<Option<Result<LoginData, String>>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let pending = Arc::new(PendingLogin {
        action_sender: action_tx,
        prompt: std::sync::Mutex::new(None),
        result: result_cell.clone(),
        finalized: std::sync::atomic::AtomicBool::new(false),
    });

    {
        let mut logins = state.pending_logins.lock().await;
        // Drop sessions that already reached a result but were never collected
        // (the user closed the tab mid-2FA), so the map can't grow without bound.
        logins.retain(|_, p| p.result.try_lock().map(|r| r.is_none()).unwrap_or(true));
        logins.insert(session_key.clone(), pending.clone());
    }

    let apple_id_clone = apple_id.clone();
    let password_clone = password.clone();
    let state_clone = state.clone();
    let result_clone = result_cell.clone();
    let pending_clone = pending.clone();

    // Spawn a thread with its own runtime so the sync 2FA callback doesn't block tokio threads.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        let outcome: Result<LoginData, String> = rt.block_on(async move {
            let anisette_url = state_clone.anisette_url.read().await.clone();
            let provider = sideload::build_anisette_provider(
                &state_clone.db,
                &apple_id_clone,
                &anisette_url,
            )
            .await
            .map_err(|e| format!("Anisette init: {e}"))?;

            let anisette_generator = isideload::anisette::AnisetteDataGenerator::new(Arc::new(
                tokio::sync::RwLock::new(provider),
            ));

            let mut account = AppleAccount::new(&apple_id_clone, anisette_generator, false)
                .await
                .map_err(|e| format!("Account init: {e}"))?;

            // The callback is re-entered for every 2FA round trip (wrong code, a
            // resend, switching methods), so the receiver lives behind a mutex to
            // satisfy the `Sync` bound on the callback type.
            let action_rx = std::sync::Mutex::new(action_rx);

            account
                .login(
                    &password_clone,
                    Box::new(move |params: TwoFactorCallbackParams| {
                        // Publish what Apple wants so the UI can render the right options.
                        *pending_clone.prompt.lock().unwrap() =
                            Some(TwoFactorPrompt::from(params));

                        // Block until the web UI answers (5-minute timeout).
                        let action = action_rx.lock().ok().and_then(|rx| {
                            rx.recv_timeout(std::time::Duration::from_secs(300)).ok()
                        });

                        match action {
                            Some(TwoFactorAction::SubmitCode(code)) => {
                                TwoFactorCallbackResponse::SubmitCode(code)
                            }
                            Some(TwoFactorAction::SendSms(id)) => {
                                TwoFactorCallbackResponse::SendSms(id)
                            }
                            Some(TwoFactorAction::SendToDevices) => {
                                TwoFactorCallbackResponse::SendToDevices
                            }
                            Some(TwoFactorAction::ResendCode) => {
                                TwoFactorCallbackResponse::ResendCode
                            }
                            // Explicit cancel, or the user walked away.
                            Some(TwoFactorAction::Abort) | None => {
                                TwoFactorCallbackResponse::Abort
                            }
                        }
                    }),
                )
                .await
                .map_err(|e| format!("Login failed: {e}"))?;

            // Serialize session.
            let spd = account
                .spd
                .clone()
                .ok_or_else(|| "No session after login".to_string())?;
            let mut spd_bytes = Vec::new();
            plist::to_writer_xml(&mut spd_bytes, &spd)
                .map_err(|e| format!("Serialize SPD: {e}"))?;

            let xcode_token =
                sideload::cache_xcode_token_from_account(&state_clone.db, &mut account)
                    .await
                    .map_err(|e| format!("Cache xcode token: {e}"))?;
            let adsid = sideload::adsid_from_spd(&spd).map_err(|e| format!("Read adsid: {e}"))?;

            // Build developer session directly from the freshly-minted token.
            let mut dev_session = DeveloperSession::new(
                xcode_token,
                adsid,
                account.grandslam_client.clone(),
                account.anisette_generator.clone(),
            );

            let teams = dev_session
                .list_teams()
                .await
                .map_err(|e| format!("ListTeams: {e}"))?;
            let first_team = teams.into_iter().next();

            Ok(LoginData {
                apple_id: apple_id_clone,
                session_plist: spd_bytes,
                team_id: first_team.as_ref().map(|t| t.team_id.clone()),
                team_name: first_team.as_ref().and_then(|t| t.name.clone()),
            })
        });

        // Write result back. The session stays in the pending map until a caller
        // observes this result, otherwise a successful login could be dropped.
        rt.block_on(async {
            *result_clone.lock().await = Some(outcome);
        });
    });

    // Wait to see if it resolves without 2FA, or asks for a code.
    let step = await_login_step(&state, &session_key, &pending).await?;
    Ok((session_key, step))
}

/// Polls a login for its next step, returning [`LoginStep::Pending`] if it is
/// still working after `WAIT`. Finalizes (and cleans up) a login that finished.
#[cfg(feature = "ssr")]
async fn await_login_step(
    state: &crate::server::state::AppState,
    session_key: &str,
    pending: &std::sync::Arc<crate::server::state::PendingLogin>,
) -> Result<LoginStep, ServerFnError> {
    use std::sync::atomic::Ordering;

    const WAIT: std::time::Duration = std::time::Duration::from_secs(10);
    const TICK: std::time::Duration = std::time::Duration::from_millis(150);

    let deadline = std::time::Instant::now() + WAIT;

    loop {
        let outcome = pending.result.lock().await.clone();
        if let Some(outcome) = outcome {
            state.pending_logins.lock().await.remove(session_key);
            let data = outcome.map_err(ServerFnError::new)?;
            // Guard against two in-flight pollers both inserting the account.
            if !pending.finalized.swap(true, Ordering::SeqCst) {
                finalize_login(state, &data).await?;
            }
            return Ok(LoginStep::Complete);
        }

        let prompt = pending.prompt.lock().unwrap().clone();
        if let Some(prompt) = prompt {
            return Ok(LoginStep::TwoFactor { prompt });
        }

        if std::time::Instant::now() >= deadline {
            return Ok(LoginStep::Pending);
        }
        tokio::time::sleep(TICK).await;
    }
}

#[cfg(feature = "ssr")]
fn lookup_login(
    logins: &std::collections::HashMap<
        String,
        std::sync::Arc<crate::server::state::PendingLogin>,
    >,
    session_key: &str,
) -> Result<std::sync::Arc<crate::server::state::PendingLogin>, ServerFnError> {
    logins
        .get(session_key)
        .cloned()
        .ok_or_else(|| ServerFnError::new("Session not found or expired"))
}

#[cfg(feature = "ssr")]
async fn finalize_login(
    state: &crate::server::state::AppState,
    data: &crate::server::state::LoginData,
) -> Result<(), ServerFnError> {
    use crate::server::db;

    let encrypted = state.crypto.encrypt(&data.session_plist);
    let id = uuid::Uuid::new_v4().to_string();

    db::insert_account(
        &state.db,
        &id,
        &data.apple_id,
        &encrypted,
        data.team_id.as_deref(),
        data.team_name.as_deref(),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Check on a login that reported [`LoginStep::Pending`].
#[server]
pub async fn poll_login(session_key: String) -> Result<LoginStep, ServerFnError> {
    use crate::server::state::AppState;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let pending = {
        let logins = state.pending_logins.lock().await;
        lookup_login(&logins, &session_key)?
    };

    await_login_step(&state, &session_key, &pending).await
}

/// Answer the outstanding [`TwoFactorPrompt`] and wait for whatever comes next.
#[server]
pub async fn submit_two_factor(
    session_key: String,
    action: TwoFactorAction,
) -> Result<LoginStep, ServerFnError> {
    use crate::server::state::AppState;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let pending = {
        let logins = state.pending_logins.lock().await;
        lookup_login(&logins, &session_key)?
    };

    // Taking the prompt both clears it and proves a callback is actually waiting,
    // so a double submit can't leave a stale action buffered for the next round.
    let waiting = pending.prompt.lock().unwrap().take().is_some();
    if !waiting {
        return Err(ServerFnError::new("No two-factor request is pending"));
    }

    let aborting = action == TwoFactorAction::Abort;

    pending
        .action_sender
        .send(action)
        .map_err(|_| ServerFnError::new("Login session closed"))?;

    // Nothing more to wait for: the login thread will unwind on its own.
    if aborting {
        state.pending_logins.lock().await.remove(&session_key);
        return Ok(LoginStep::Pending);
    }

    await_login_step(&state, &session_key, &pending).await
}

#[server]
pub async fn delete_account(id: String) -> Result<(), ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    db::delete_account(&state.db, &id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn export_livecontainer_cert(
    account_id: String,
) -> Result<LcCertExport, ServerFnError> {
    use base64::prelude::*;
    use crate::server::{db, sideload, state::AppState};
    use crate::server::db::storage::DbStorage;
    use isideload::{
        dev::teams::TeamsApi,
        sideload::{builder::MaxCertsBehavior, cert_identity::CertificateIdentity},
    };
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let row = db::list_accounts(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .find(|r| r.id == account_id)
        .ok_or_else(|| ServerFnError::new("Account not found"))?;

    let encrypted_spd = sqlx::query_scalar!(
        "SELECT session_blob FROM apple_accounts WHERE id = ?",
        account_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("No session blob found for account"))?;

    let anisette_url = state.anisette_url.read().await.clone();
    let mut dev_session =
        sideload::get_dev_session(
            &state.db,
            &row.apple_id,
            &encrypted_spd,
            &state.crypto,
            &anisette_url,
        )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    let teams = dev_session
        .list_teams()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let team = teams
        .into_iter()
        .next()
        .ok_or_else(|| ServerFnError::new("No developer team found for this account"))?;

    let db_storage = DbStorage::load(
        state.db.clone(),
        &sideload::account_storage_prefix(&row.apple_id),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cert = CertificateIdentity::retrieve(
        "jas",
        &row.apple_id,
        &mut dev_session,
        &team,
        &db_storage,
        &MaxCertsBehavior::Revoke,
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to retrieve certificate: {e}")))?;

    let p12_bytes = cert
        .as_p12(&cert.machine_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to export P12: {e}")))?;

    Ok(LcCertExport {
        p12_b64: BASE64_STANDARD.encode(&p12_bytes),
        serial: cert.get_serial_number(),
        password: cert.machine_id.clone(),
    })
}

#[server]
pub async fn list_account_app_ids(account_id: String) -> Result<AppIdsResult, ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use isideload::dev::{app_ids::AppIdsApi, teams::TeamsApi};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let row = db::list_accounts(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .find(|r| r.id == account_id)
        .ok_or_else(|| ServerFnError::new("Account not found"))?;

    let encrypted_spd = sqlx::query_scalar!(
        "SELECT session_blob FROM apple_accounts WHERE id = ?",
        account_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("No session blob found for account"))?;

    let anisette_url = state.anisette_url.read().await.clone();
    let mut dev_session =
        sideload::get_dev_session(
            &state.db,
            &row.apple_id,
            &encrypted_spd,
            &state.crypto,
            &anisette_url,
        )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    let teams = dev_session
        .list_teams()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let team = teams
        .into_iter()
        .next()
        .ok_or_else(|| ServerFnError::new("No developer team found for this account"))?;

    let resp = dev_session
        .list_app_ids(&team, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let max_quantity = resp.max_quantity;
    let available_quantity = resp.available_quantity;
    let entries = resp
        .app_ids
        .into_iter()
        .map(|a| {
            let expiration_date = a.expiration_date.map(|d| {
                let st: std::time::SystemTime = d.into();
                let dt: chrono::DateTime<chrono::Utc> = st.into();
                dt.timestamp()
            });
            AppIdEntry {
                name: a.name,
                identifier: a.identifier,
                expiration_date,
            }
        })
        .collect();

    Ok(AppIdsResult {
        entries,
        max_quantity,
        available_quantity,
    })
}

#[server]
pub async fn list_apps(device_id: String) -> Result<Vec<AppInfo>, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let rows = db::list_apps_for_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| AppInfo {
            id: r.id,
            device_id: r.device_id,
            bundle_id: r.bundle_id,
            display_name: r.display_name,
            version: r.version,
            has_ipa: r.ipa_path.is_some(),
            installed_at: r.installed_at,
            last_refreshed: r.last_refreshed,
            refresh_enabled: r.refresh_enabled,
        })
        .collect())
}

/// Connect to the device, query instproxy, and remove any confirmed-installed
/// app rows whose bundle ID is no longer present. Returns the number of rows
/// pruned.
#[server]
pub async fn reconcile_apps(device_id: String) -> Result<usize, ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let device = db::get_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Device not found"))?;

    let pairing_bytes = state
        .crypto
        .decrypt(&device.pairing_blob)
        .map_err(|_| ServerFnError::new("Failed to decrypt pairing"))?;

    let installed =
        sideload::list_installed_bundle_ids(&device.ip, device.mdns_ip.as_deref(), &pairing_bytes)
            .await
            .map_err(|e| ServerFnError::new(format!("Device unreachable: {e}")))?;

    if installed.is_empty() {
        return Err(ServerFnError::new("Device returned no apps"));
    }

    let rows = db::list_apps_for_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut pruned = 0usize;
    for r in rows {
        if r.installed_at.is_some() && !installed.contains(&r.bundle_id) {
            tracing::info!(
                "Pruning {} ({}): not present on device",
                r.display_name,
                r.bundle_id
            );
            db::delete_app(&state.db, &r.id)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            pruned += 1;
        }
    }
    Ok(pruned)
}

#[server]
pub async fn install_ipa(
    device_id: String,
    account_id: String,
    ipa_bytes_b64: String,
    store: bool,
) -> Result<String, ServerFnError> {
    use crate::server::{
        db, sideload,
        state::{AppState, JobKind, JobRequest},
    };
    use base64::Engine;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    if device_id.is_empty() {
        return Err(ServerFnError::new(
            "device_id is empty, form submission bug",
        ));
    }
    if account_id.is_empty() {
        return Err(ServerFnError::new("account_id is empty, select an account"));
    }

    // Verify the device exists and is reachable before doing any work. A quick
    // TCP probe here means a disconnected device fails fast with a clear error
    // instead of ~10s into a sign+install.
    let device = db::get_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new(format!("Device '{device_id}' not found in DB")))?;

    sideload::tcp_ping(&device.ip, device.mdns_ip.as_deref())
        .await
        .map_err(|e| {
            ServerFnError::new(format!(
                "Device is not reachable ({e}). Make sure it is powered on and on the same network, then try again."
            ))
        })?;

    let account_exists = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM apple_accounts WHERE id = ?",
        account_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if account_exists == 0 {
        return Err(ServerFnError::new(format!(
            "Account '{account_id}' not found in DB"
        )));
    }

    let ipa_bytes = base64::engine::general_purpose::STANDARD
        .decode(&ipa_bytes_b64)
        .map_err(|e| ServerFnError::new(format!("Invalid IPA data: {e}")))?;

    if ipa_bytes.is_empty() {
        return Err(ServerFnError::new(
            "IPA file is empty, select a valid .ipa before submitting",
        ));
    }
    if ipa_bytes.get(..4) != Some(&[0x50, 0x4B, 0x03, 0x04]) {
        return Err(ServerFnError::new(
            "File does not look like a valid IPA (wrong magic bytes)",
        ));
    }

    let info = sideload::read_ipa_info(&ipa_bytes)
        .map_err(|e| ServerFnError::new(format!("Could not read IPA metadata: {e}")))?;

    // Optionally persist the IPA.
    let ipa_path = if store {
        let dir = &state.config.storage.ipa_dir;
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let filename = format!("{dir}/{}.ipa", info.bundle_id);
        tokio::fs::write(&filename, &ipa_bytes)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Some(filename)
    } else {
        let tmp = format!("/tmp/jas_{}.ipa", uuid::Uuid::new_v4());
        tokio::fs::write(&tmp, &ipa_bytes)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Some(tmp)
    };

    let proposed_app_id = uuid::Uuid::new_v4().to_string();
    let app_id = db::upsert_app(
        &state.db,
        &proposed_app_id,
        &device_id,
        &info.bundle_id,
        &info.display_name,
        info.version.as_deref(),
        ipa_path.as_deref(),
        Some(&account_id),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("upsert_app failed: {e}")))?;

    let job_id = uuid::Uuid::new_v4().to_string();
    db::insert_job(&state.db, &job_id, &app_id, "install")
        .await
        .map_err(|e| ServerFnError::new(format!("insert_job failed (app_id={app_id}): {e}")))?;

    let ipa_for_job = ipa_path.unwrap_or_default();
    let req = JobRequest {
        job_id: job_id.clone(),
        app_id,
        device_id: device_id.clone(),
        account_id,
        kind: JobKind::Install {
            ipa_path: ipa_for_job,
        },
    };

    state
        .job_tx
        .send(req)
        .await
        .map_err(|_| ServerFnError::new("Job queue full"))?;

    Ok(job_id)
}

#[server]
pub async fn refresh_app(app_id: String) -> Result<String, ServerFnError> {
    use crate::server::{
        db,
        state::{AppState, JobKind, JobRequest},
    };
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let app = db::get_app(&state.db, &app_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("App not found"))?;

    if app.ipa_path.is_none() {
        return Err(ServerFnError::new("No stored IPA, cannot refresh"));
    }

    let account_id = app
        .account_id
        .ok_or_else(|| ServerFnError::new("No account linked to this app"))?;

    let job_id = uuid::Uuid::new_v4().to_string();
    db::insert_job(&state.db, &job_id, &app_id, "refresh")
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    state
        .job_tx
        .send(JobRequest {
            job_id: job_id.clone(),
            app_id,
            device_id: app.device_id,
            account_id,
            kind: JobKind::Refresh,
        })
        .await
        .map_err(|_| ServerFnError::new("Job queue full"))?;

    Ok(job_id)
}

/// Enqueue a refresh job for every refresh-enabled, IPA-backed app on this
/// device that has an Apple account linked. Returns the number of jobs queued.
#[server]
pub async fn refresh_device(device_id: String) -> Result<usize, ServerFnError> {
    use crate::server::{
        db,
        state::{AppState, JobKind, JobRequest},
    };
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let apps = db::list_apps_for_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut queued = 0usize;
    for app in apps {
        if !app.refresh_enabled || app.ipa_path.is_none() || app.installed_at.is_none() {
            continue;
        }
        let Some(account_id) = app.account_id.clone() else {
            continue;
        };

        let job_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = db::insert_job(&state.db, &job_id, &app.id, "refresh").await {
            tracing::warn!("refresh_device: insert_job for {} failed: {e}", app.id);
            continue;
        }

        let req = JobRequest {
            job_id,
            app_id: app.id.clone(),
            device_id: app.device_id.clone(),
            account_id,
            kind: JobKind::Refresh,
        };

        match state.job_tx.send(req).await {
            Ok(()) => queued += 1,
            Err(_) => return Err(ServerFnError::new("Job queue full")),
        }
    }

    Ok(queued)
}

#[server]
pub async fn list_jobs() -> Result<Vec<QueueEntry>, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let rows = db::list_jobs(&state.db, 20)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| QueueEntry {
            id: r.id,
            app_id: r.app_id,
            app_name: r.app_name,
            device_id: r.device_id,
            device_name: r.device_name,
            kind: r.kind,
            status: r.status,
            error: r.error,
            started_at: r.started_at,
            finished_at: r.finished_at,
            progress: r.progress,
            stage: r.stage,
        })
        .collect())
}

#[server]
pub async fn job_status(job_id: String) -> Result<JobInfo, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let row = db::get_job(&state.db, &job_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Job not found"))?;

    Ok(JobInfo {
        id: row.id,
        app_id: row.app_id,
        kind: row.kind,
        status: row.status,
        error: row.error,
        started_at: row.started_at,
        finished_at: row.finished_at,
        progress: row.progress,
        stage: row.stage,
    })
}

#[server]
pub async fn set_refresh_enabled(app_id: String, enabled: bool) -> Result<(), ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    db::set_app_refresh(&state.db, &app_id, enabled)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn delete_app(app_id: String) -> Result<(), ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let app = db::get_app(&state.db, &app_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("App not found"))?;

    // Skip the device round-trip when the install never finished. There's
    // nothing on the device to remove, so just drop the DB row.
    if app.installed_at.is_some() {
        let device = db::get_device(&state.db, &app.device_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("Device not found"))?;

        let pairing_bytes = state
            .crypto
            .decrypt(&device.pairing_blob)
            .map_err(|_| ServerFnError::new("Failed to decrypt pairing"))?;

        sideload::uninstall_app(
            &device.ip,
            device.mdns_ip.as_deref(),
            &pairing_bytes,
            &app.bundle_id,
        )
        .await
        .map_err(|e| ServerFnError::new(format!("Uninstall failed: {e}")))?;
    }

    db::delete_app(&state.db, &app_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Return the decrypted pairing plist as base64 so the client can download it.
#[server]
pub async fn export_pairing(device_id: String) -> Result<String, ServerFnError> {
    use base64::prelude::*;
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let device = db::get_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Device not found"))?;

    let bytes = state
        .crypto
        .decrypt(&device.pairing_blob)
        .map_err(|_| ServerFnError::new("Failed to decrypt pairing"))?;

    Ok(BASE64_STANDARD.encode(&bytes))
}

/// Replace a device's pairing file after validating the new plist.
#[server]
pub async fn reimport_pairing(
    device_id: String,
    pairing_blob_b64: String,
) -> Result<(), ServerFnError> {
    use base64::prelude::*;
    use crate::server::{db, sideload, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    db::get_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Device not found"))?;

    let bytes = BASE64_STANDARD
        .decode(&pairing_blob_b64)
        .map_err(|e| ServerFnError::new(format!("Invalid base64: {e}")))?;

    sideload::validate_pairing_bytes(&bytes).map_err(|e| ServerFnError::new(e.to_string()))?;

    let encrypted = state.crypto.encrypt(&bytes);
    db::update_device_pairing(&state.db, &device_id, &encrypted)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Update the static IP stored for a device.
#[server]
pub async fn change_device_ip(device_id: String, ip: String) -> Result<(), ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let ip = ip.trim().to_string();
    if ip.is_empty() {
        return Err(ServerFnError::new("IP must not be empty"));
    }
    db::update_device_ip(&state.db, &device_id, &ip)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Revoke all iOS development certificates for an account.
/// Returns the number of certs revoked.
#[server]
pub async fn revoke_all_certs(account_id: String) -> Result<usize, ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use isideload::dev::{certificates::CertificatesApi, teams::TeamsApi};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let row = db::list_accounts(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .find(|r| r.id == account_id)
        .ok_or_else(|| ServerFnError::new("Account not found"))?;

    let encrypted_spd = sqlx::query_scalar!(
        "SELECT session_blob FROM apple_accounts WHERE id = ?",
        account_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("No session blob found for account"))?;

    let anisette_url = state.anisette_url.read().await.clone();
    let mut dev_session =
        sideload::get_dev_session(
            &state.db,
            &row.apple_id,
            &encrypted_spd,
            &state.crypto,
            &anisette_url,
        )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    let teams = dev_session
        .list_teams()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let team = teams
        .into_iter()
        .next()
        .ok_or_else(|| ServerFnError::new("No developer team found for this account"))?;

    let certs = dev_session
        .list_ios_certs(&team)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let total = certs.len();
    let mut revoked = 0usize;

    for cert in certs {
        if let Some(serial) = cert.serial_number {
            if let Err(e) = dev_session
                .revoke_development_cert(&team, &serial, None)
                .await
            {
                tracing::warn!("Failed to revoke cert {serial}: {e}");
            } else {
                revoked += 1;
            }
        }
    }

    tracing::info!(
        "Revoked {revoked}/{total} iOS dev certs for {}",
        row.apple_id
    );
    Ok(revoked)
}

#[server]
pub async fn get_device_info(id: String) -> Result<DeviceInfo, ServerFnError> {
    use crate::server::{db, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let row = db::get_device(&state.db, &id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Device not found"))?;

    Ok(DeviceInfo {
        id: row.id,
        name: row.name,
        udid: row.udid,
        ip: row.ip,
        port: row.port,
        last_seen: row.last_seen,
        discovery: row.discovery,
        mdns_ip: row.mdns_ip,
        mdns_seen_at: row.mdns_seen_at,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceProbe {
    /// Round-trip time in ms if a live TCP probe to the device succeeded just now.
    pub reachable_ms: Option<u64>,
    pub mdns_seen_at: Option<i64>,
}

/// Actively TCP-probes the device (preferring its mDNS LAN IP, falling back
/// to the manual IP) so the UI can show real-time reachability instead of
/// relying solely on the last mDNS sighting, which never fires for
/// VPN-only devices.
#[server]
pub async fn probe_device(device_id: String) -> Result<DeviceProbe, ServerFnError> {
    use crate::server::{db, sideload, state::AppState};
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let device = db::get_device(&state.db, &device_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Device not found"))?;

    let reachable_ms = sideload::tcp_ping(&device.ip, device.mdns_ip.as_deref())
        .await
        .ok();

    Ok(DeviceProbe {
        reachable_ms,
        mdns_seen_at: device.mdns_seen_at,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfigInfo {
    pub bind: String,
    pub log_level: String,
    pub anisette_url: String,
    pub database_path: String,
    pub ipa_dir: String,
    pub interval_hours: u64,
    pub refresh_window_days: i64,
    pub worker_threads: usize,
    pub mdns_enabled: bool,
    pub mdns_interface: String,
}

#[server]
pub async fn get_server_config() -> Result<ServerConfigInfo, ServerFnError> {
    use crate::server::state::AppState;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;
    let cfg = &state.config;
    let anisette_url = state.anisette_url.read().await.clone();
    Ok(ServerConfigInfo {
        bind: cfg.server.bind.clone(),
        log_level: cfg.server.log_level.clone(),
        anisette_url,
        database_path: cfg.storage.database_path.clone(),
        ipa_dir: cfg.storage.ipa_dir.clone(),
        interval_hours: cfg.scheduler.interval_hours,
        refresh_window_days: cfg.scheduler.refresh_window_days,
        worker_threads: cfg.scheduler.worker_threads,
        mdns_enabled: cfg.discovery.mdns_enabled,
        mdns_interface: cfg.discovery.mdns_interface.clone(),
    })
}

#[server]
pub async fn set_anisette_url(url: String) -> Result<(), ServerFnError> {
    use crate::server::state::AppState;
    use leptos::prelude::use_context;

    let state = use_context::<AppState>().ok_or_else(|| ServerFnError::new("No state"))?;

    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(ServerFnError::new("URL must not be empty"));
    }

    sqlx::query!(
        "INSERT INTO sideload_storage (key, value) VALUES ('_server/anisette_url', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        url
    )
    .execute(&state.db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    *state.anisette_url.write().await = url;
    Ok(())
}
