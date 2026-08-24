use std::{collections::HashMap, sync::atomic::AtomicBool, sync::Arc};
use tokio::sync::{Mutex, RwLock, mpsc};
use sqlx::SqlitePool;

use crate::server::{config::Config, crypto::Crypto};

/// A login in flight, possibly parked waiting on the user for two-factor auth.
pub struct PendingLogin {
    /// Delivers the user's answer to the login thread blocked in the 2FA callback.
    pub action_sender: std::sync::mpsc::SyncSender<crate::app::TwoFactorAction>,
    /// What Apple is currently asking for. Set by the login thread before it
    /// blocks; taken by whoever answers it.
    pub prompt: std::sync::Mutex<Option<crate::app::TwoFactorPrompt>>,
    pub result: Arc<Mutex<Option<Result<LoginData, String>>>>,
    /// Set once the account has been written to the database, so concurrent
    /// pollers can't insert it twice.
    pub finalized: AtomicBool,
}

#[derive(Clone)]
pub struct LoginData {
    pub apple_id: String,
    pub session_plist: Vec<u8>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
}

/// Background job request.
pub struct JobRequest {
    pub job_id: String,
    pub app_id: String,
    pub device_id: String,
    pub account_id: String,
    pub kind: JobKind,
}

pub enum JobKind {
    Install { ipa_path: String },
    Refresh,
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub crypto: Crypto,
    pub job_tx: mpsc::Sender<JobRequest>,
    pub pending_logins: Arc<Mutex<HashMap<String, Arc<PendingLogin>>>>,
    pub anisette_url: Arc<RwLock<String>>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        config: Config,
        key: [u8; 32],
        job_tx: mpsc::Sender<JobRequest>,
        anisette_url: String,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            crypto: Crypto::new(key),
            job_tx,
            pending_logins: Arc::new(Mutex::new(HashMap::new())),
            anisette_url: Arc::new(RwLock::new(anisette_url)),
        }
    }
}
