use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tracing::info;

use idevice::{
    afc::opcode::AfcFopenMode,
    afc::AfcClient,
    installation_proxy::InstallationProxyClient,
    misagent::MisagentClient,
    remote_pairing::{
        connect_tls_psk_tunnel_native, RemotePairingClient, RpPairingFile, RpPairingSocket,
    },
    rsd::RsdHandshake,
    services::lockdown::LockdownClient,
    tcp, RsdService,
};
use isideload::{
    anisette::{remote_v3::RemoteV3AnisetteProvider, AnisetteDataGenerator},
    auth::{
        apple_account::{AppToken, AppleAccount},
        grandslam::GrandSlam,
    },
    dev::{
        app_ids::AppIdsApi, developer_session::DeveloperSession, devices::DevicesApi,
        teams::TeamsApi,
    },
    sideload::{builder::MaxCertsBehavior, SideloaderBuilder, TeamSelection},
    util::storage::SideloadingStorage,
};
use std::io::Write;

use crate::server::{crypto::Crypto, db::storage::DbStorage};

async fn open_cdtunnel(
    device_ip: &str,
    pairing_bytes: &[u8],
) -> anyhow::Result<(tcp::handle::AdapterHandle, RsdHandshake)> {
    let mut rpf = RpPairingFile::from_bytes(pairing_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid pairing file: {e:?}"))?;

    info!("Connecting to RPPairing at {device_ip}:49152");
    let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect((device_ip, 49152u16)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("TCP connect to {device_ip}:49152 timed out"))?
    .map_err(|e| anyhow::anyhow!("TCP connect to {device_ip}:49152 failed: {e}"))?;

    let socket = RpPairingSocket::new(stream);
    let mut rpc = RemotePairingClient::new(socket, "jas");

    rpc.attempt_pair_verify()
        .await
        .map_err(|e| anyhow::anyhow!("RPPairing handshake failed: {e:?}"))?;
    rpc.validate_pairing(&mut rpf).await.map_err(|e| {
        anyhow::anyhow!("RPPairing verification failed (wrong pairing file?): {e:?}")
    })?;

    let tunnel_port = rpc
        .create_tcp_listener()
        .await
        .map_err(|e| anyhow::anyhow!("create_tcp_listener failed: {e:?}"))?;
    info!("CDTunnel listener on {device_ip}:{tunnel_port}");

    let tunnel_stream = tokio::net::TcpStream::connect((device_ip, tunnel_port))
        .await
        .map_err(|e| anyhow::anyhow!("CDTunnel TCP connect to port {tunnel_port} failed: {e}"))?;

    let tunnel = connect_tls_psk_tunnel_native(tunnel_stream, rpc.encryption_key())
        .await
        .map_err(|e| anyhow::anyhow!("TLS-PSK tunnel failed: {e:?}"))?;

    let client_ip: std::net::IpAddr = tunnel
        .info
        .client_address
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid tunnel client IP: {e}"))?;
    let server_ip: std::net::IpAddr = tunnel
        .info
        .server_address
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid tunnel server IP: {e}"))?;
    let mtu = tunnel.info.mtu;
    let rsd_port = tunnel.info.server_rsd_port;
    info!("CDTunnel up: client={client_ip} server={server_ip} rsd={rsd_port} mtu={mtu}");

    let raw = tunnel.into_inner();
    let mut adapter = tcp::adapter::Adapter::new(Box::new(raw), client_ip, server_ip);
    adapter.set_mss((mtu as usize).saturating_sub(60));
    let mut handle: tcp::handle::AdapterHandle = adapter.to_async_handle();

    let rsd_stream = handle
        .connect(rsd_port)
        .await
        .map_err(|e| anyhow::anyhow!("RSD connect failed: {e:?}"))?;
    let handshake = RsdHandshake::new(rsd_stream)
        .await
        .map_err(|e| anyhow::anyhow!("RSD handshake failed: {e:?}"))?;

    Ok((handle, handshake))
}

async fn open_cdtunnel_preferring_mdns(
    manual_ip: &str,
    mdns_ip: Option<&str>,
    pairing_bytes: &[u8],
) -> anyhow::Result<(tcp::handle::AdapterHandle, RsdHandshake)> {
    if let Some(ip) = mdns_ip {
        if ip != manual_ip {
            match open_cdtunnel(ip, pairing_bytes).await {
                Ok(out) => return Ok(out),
                Err(e) => {
                    info!("mDNS IP {ip} failed ({e}); falling back to manual IP {manual_ip}");
                }
            }
        }
    }
    open_cdtunnel(manual_ip, pairing_bytes).await
}

pub async fn list_installed_bundle_ids(
    device_ip: &str,
    mdns_ip: Option<&str>,
    pairing_bytes: &[u8],
) -> anyhow::Result<std::collections::HashSet<String>> {
    let (mut handle, mut handshake) =
        open_cdtunnel_preferring_mdns(device_ip, mdns_ip, pairing_bytes).await?;

    let mut instproxy = InstallationProxyClient::connect_rsd(&mut handle, &mut handshake)
        .await
        .map_err(|e| anyhow::anyhow!("InstallationProxy connect failed: {e:?}"))?;

    let entries = instproxy
        .browse(None)
        .await
        .map_err(|e| anyhow::anyhow!("browse failed: {e:?}"))?;

    let mut ids = std::collections::HashSet::new();
    for entry in entries {
        if let plist::Value::Dictionary(dict) = entry {
            if let Some(plist::Value::String(bid)) = dict.get("CFBundleIdentifier") {
                ids.insert(bid.clone());
            }
        }
    }

    info!("Device {device_ip} reports {} installed apps", ids.len());
    Ok(ids)
}

pub async fn uninstall_app(
    device_ip: &str,
    mdns_ip: Option<&str>,
    pairing_bytes: &[u8],
    bundle_id: &str,
) -> anyhow::Result<()> {
    let (mut handle, mut handshake) =
        open_cdtunnel_preferring_mdns(device_ip, mdns_ip, pairing_bytes).await?;

    let mut instproxy = InstallationProxyClient::connect_rsd(&mut handle, &mut handshake)
        .await
        .map_err(|e| anyhow::anyhow!("InstallationProxy connect failed: {e:?}"))?;

    info!("Uninstalling {bundle_id} from {device_ip}");
    instproxy
        .uninstall(bundle_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("instproxy uninstall failed: {e:?}"))?;

    info!("Uninstall complete: {bundle_id}");
    Ok(())
}

/// Connect to a device, verify the pairing file, and return its UDID and name via lockdown
pub async fn fetch_device_identity(
    device_ip: &str,
    pairing_bytes: &[u8],
) -> anyhow::Result<(String, String)> {
    let (mut handle, mut handshake) = open_cdtunnel(device_ip, pairing_bytes).await?;

    let mut lockdown = handshake
        .connect::<LockdownClient>(&mut handle)
        .await
        .map_err(|e| anyhow::anyhow!("Lockdown connect via RSD failed: {e:?}"))?;

    let udid = lockdown
        .get_value(Some("UniqueDeviceID"), None::<&str>)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read UDID from lockdown: {e:?}"))?
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("UniqueDeviceID is not a string"))?
        .to_string();

    let name = lockdown
        .get_value(Some("DeviceName"), None::<&str>)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read DeviceName from lockdown: {e:?}"))?
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("DeviceName is not a string"))?
        .to_string();

    Ok((udid, name))
}

/// TCP-level reachability check on port 49152. Returns round-trip ms.
pub async fn tcp_ping(device_ip: &str, mdns_ip: Option<&str>) -> anyhow::Result<u64> {
    let ip = mdns_ip.filter(|&m| m != device_ip).unwrap_or(device_ip);
    let start = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect((ip, 49152u16)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("TCP connect to {ip}:49152 timed out"))?
    .map_err(|e| anyhow::anyhow!("TCP connect to {ip}:49152 failed: {e}"))?;
    Ok(start.elapsed().as_millis() as u64)
}

/// Parse a pairing plist to confirm it is structurally valid.
pub fn validate_pairing_bytes(bytes: &[u8]) -> anyhow::Result<()> {
    RpPairingFile::from_bytes(bytes)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Invalid pairing file: {e:?}"))
}

/// Storage prefix used to scope anisette state (and other sideload storage)
/// to a single Apple ID. Stable across runs so the persisted anisette identity
/// matches the one the cached SPD/GsIdmsToken was minted against.
pub fn account_storage_prefix(apple_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(apple_id.as_bytes());
    hex::encode(h.finalize())
}

/// Build an anisette provider that persists its state in our SQLite DB,
/// scoped to `apple_id`
pub async fn build_anisette_provider(
    pool: &SqlitePool,
    apple_id: &str,
    anisette_url: &str,
) -> anyhow::Result<RemoteV3AnisetteProvider> {
    let prefix = account_storage_prefix(apple_id);
    let storage = DbStorage::load(pool.clone(), &prefix)
        .await
        .map_err(|e| anyhow::anyhow!("Anisette storage load failed: {e}"))?;

    Ok(RemoteV3AnisetteProvider::default()
        .map_err(|e| anyhow::anyhow!("Anisette init failed: {e}"))?
        .set_url(anisette_url)
        .set_storage(Box::new(storage)))
}

pub async fn restore_account(
    pool: &SqlitePool,
    apple_id: &str,
    encrypted_spd: &[u8],
    crypto: &Crypto,
    anisette_url: &str,
) -> anyhow::Result<AppleAccount> {
    let spd_bytes = crypto
        .decrypt(encrypted_spd)
        .map_err(|_| anyhow::anyhow!("Failed to decrypt account session"))?;

    let provider = build_anisette_provider(pool, apple_id, anisette_url).await?;

    let anisette_generator = isideload::anisette::AnisetteDataGenerator::new(std::sync::Arc::new(
        tokio::sync::RwLock::new(provider),
    ));

    let mut account = AppleAccount::new(apple_id, anisette_generator, false)
        .await
        .map_err(|e| anyhow::anyhow!("AppleAccount init failed: {e}"))?;

    let spd: plist::Dictionary = plist::from_bytes(&spd_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse SPD plist: {e}"))?;
    account.spd = Some(spd);

    Ok(account)
}

const XCODE_TOKEN_KEY: &str = "xcode_auth_token_v1";

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedXcodeToken {
    token: String,
    adsid: String,
    expiry: u64,
    duration: u64,
}

impl CachedXcodeToken {
    fn is_fresh(&self) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(u64::MAX);
        self.expiry > now_ms.saturating_add(60_000)
    }
}

async fn cache_xcode_token(
    pool: &SqlitePool,
    apple_id: &str,
    token: &AppToken,
    adsid: &str,
) -> anyhow::Result<()> {
    let storage = DbStorage::load(pool.clone(), &account_storage_prefix(apple_id))
        .await
        .map_err(|e| anyhow::anyhow!("Storage load failed: {e}"))?;
    let cached = CachedXcodeToken {
        token: token.token.clone(),
        adsid: adsid.to_string(),
        expiry: token.expiry,
        duration: token.duration,
    };
    let json = serde_json::to_vec(&cached)?;
    storage
        .store_data(XCODE_TOKEN_KEY, &json)
        .map_err(|e| anyhow::anyhow!("Failed to cache xcode token: {e}"))?;
    Ok(())
}

pub fn adsid_from_spd(spd: &plist::Dictionary) -> anyhow::Result<String> {
    spd.get("adsid")
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("adsid missing from SPD"))
}

pub async fn cache_xcode_token_from_account(
    pool: &SqlitePool,
    account: &mut AppleAccount,
) -> anyhow::Result<AppToken> {
    let token = account
        .get_app_token("xcode.auth")
        .await
        .map_err(|e| anyhow::anyhow!("get_app_token(xcode.auth) failed: {e}"))?;
    let adsid = account
        .spd
        .as_ref()
        .map(adsid_from_spd)
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("SPD not loaded on account"))?;
    cache_xcode_token(pool, &account.email, &token, &adsid).await?;
    Ok(token)
}

pub async fn get_dev_session(
    pool: &SqlitePool,
    apple_id: &str,
    encrypted_spd: &[u8],
    crypto: &Crypto,
    anisette_url: &str,
) -> anyhow::Result<DeveloperSession> {
    let storage = DbStorage::load(pool.clone(), &account_storage_prefix(apple_id))
        .await
        .map_err(|e| anyhow::anyhow!("Storage load failed: {e}"))?;

    let provider = build_anisette_provider(pool, apple_id, anisette_url).await?;
    let anisette_generator =
        AnisetteDataGenerator::new(std::sync::Arc::new(tokio::sync::RwLock::new(provider)));

    if let Ok(Some(blob)) = storage.retrieve_data(XCODE_TOKEN_KEY) {
        match serde_json::from_slice::<CachedXcodeToken>(&blob) {
            Ok(cached) if cached.is_fresh() => {
                let client_info = anisette_generator
                    .get_client_info()
                    .await
                    .map_err(|e| anyhow::anyhow!("Anisette get_client_info failed: {e}"))?;
                let grandslam = std::sync::Arc::new(
                    GrandSlam::new(client_info, false)
                        .await
                        .map_err(|e| anyhow::anyhow!("GrandSlam init failed: {e}"))?,
                );
                let adsid = cached.adsid.clone();
                let token = AppToken {
                    token: cached.token,
                    duration: cached.duration,
                    expiry: cached.expiry,
                };
                info!("Using cached xcode.auth token for {apple_id}");
                return Ok(DeveloperSession::new(
                    token,
                    adsid,
                    grandslam,
                    anisette_generator,
                ));
            }
            Ok(_) => info!("Cached xcode.auth token expired, re-minting via SPD"),
            Err(e) => tracing::warn!("Cached xcode token unparseable, re-minting: {e}"),
        }
    }

    // if failure, remint xcode token
    let mut account = restore_account(pool, apple_id, encrypted_spd, crypto, anisette_url).await?;
    let token = match cache_xcode_token_from_account(pool, &mut account).await {
        Ok(t) => t,
        Err(e) => return Err(e),
    };
    let adsid = adsid_from_spd(
        account
            .spd
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("SPD not loaded"))?,
    )?;
    Ok(DeveloperSession::new(
        token,
        adsid,
        account.grandslam_client.clone(),
        account.anisette_generator.clone(),
    ))
}

fn create_ipa(app_path: &Path) -> anyhow::Result<Vec<u8>> {
    let app_name = app_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("app_path has no filename"))?
        .to_string_lossy();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    add_to_zip(&mut zip, app_path, &format!("Payload/{}", app_name))?;
    let cursor = zip
        .finish()
        .map_err(|e| anyhow::anyhow!("Failed to finalize IPA zip: {e}"))?;
    Ok(cursor.into_inner())
}

fn add_to_zip(
    zip: &mut zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
    dir: &Path,
    prefix: &str,
) -> anyhow::Result<()> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let zip_path = format!("{}/{}", prefix, name);

        if path.is_dir() {
            zip.add_directory(&zip_path, options)
                .map_err(|e| anyhow::anyhow!("Failed to add directory to IPA zip: {e}"))?;
            add_to_zip(zip, &path, &zip_path)?;
        } else {
            zip.start_file(&zip_path, options)
                .map_err(|e| anyhow::anyhow!("Failed to start file in IPA zip: {e}"))?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}

pub struct IpaInfo {
    pub bundle_id: String,
    pub display_name: String,
    pub version: Option<String>,
}

pub fn read_ipa_info(ipa_bytes: &[u8]) -> anyhow::Result<IpaInfo> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(ipa_bytes))
        .map_err(|e| anyhow::anyhow!("IPA is not a valid zip: {e}"))?;

    // Find Payload/<Something>.app/Info.plist (only the top-level one).
    let info_name = {
        let mut found = None;
        for i in 0..archive.len() {
            let name = archive
                .by_index(i)
                .map_err(|e| anyhow::anyhow!("zip entry {i}: {e}"))?
                .name()
                .to_string();
            if let Some(rest) = name.strip_prefix("Payload/") {
                if let Some((app_dir, tail)) = rest.split_once('/') {
                    if app_dir.ends_with(".app") && tail == "Info.plist" {
                        found = Some(name);
                        break;
                    }
                }
            }
        }
        found.ok_or_else(|| anyhow::anyhow!("Info.plist not found in IPA"))?
    };

    let mut buf = Vec::new();
    archive
        .by_name(&info_name)
        .map_err(|e| anyhow::anyhow!("open {info_name}: {e}"))?
        .read_to_end(&mut buf)?;

    let dict: plist::Dictionary =
        plist::from_bytes(&buf).map_err(|e| anyhow::anyhow!("parse Info.plist: {e}"))?;

    let bundle_id = dict
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .ok_or_else(|| anyhow::anyhow!("CFBundleIdentifier missing"))?
        .to_string();

    let display_name = dict
        .get("CFBundleDisplayName")
        .and_then(|v| v.as_string())
        .or_else(|| dict.get("CFBundleName").and_then(|v| v.as_string()))
        .unwrap_or(&bundle_id)
        .to_string();

    let version = dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .or_else(|| dict.get("CFBundleVersion").and_then(|v| v.as_string()))
        .map(|s| s.to_string());

    Ok(IpaInfo {
        bundle_id,
        display_name,
        version,
    })
}

fn read_bundle_id_from_app(app_path: &Path) -> anyhow::Result<String> {
    let info_path = app_path.join("Info.plist");
    let bytes = std::fs::read(&info_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", info_path.display()))?;
    let dict: plist::Dictionary =
        plist::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("parse Info.plist: {e}"))?;
    dict.get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("CFBundleIdentifier missing from Info.plist"))
}

#[allow(clippy::too_many_arguments)]
pub async fn install_ipa(
    pool: &SqlitePool,
    crypto: &Crypto,
    apple_id: &str,
    encrypted_spd: &[u8],
    device_ip: &str,
    mdns_ip: Option<&str>,
    device_name: &str,
    device_udid: &str,
    encrypted_pairing: &[u8],
    ipa_path: &str,
    anisette_url: &str,
    progress_cb: impl Fn(u64, &'static str) + Send + 'static,
) -> anyhow::Result<String> {
    let db_storage = DbStorage::load(pool.clone(), &account_storage_prefix(apple_id))
        .await
        .map_err(|e| anyhow::anyhow!("Storage load failed: {e}"))?;

    let dev_session = get_dev_session(pool, apple_id, encrypted_spd, crypto, anisette_url).await?;

    let mut sideloader = SideloaderBuilder::new(dev_session, apple_id.to_string())
        .team_selection(TeamSelection::First)
        .max_certs_behavior(MaxCertsBehavior::Revoke)
        .storage(Box::new(db_storage))
        .machine_name("jas".to_string())
        .build();

    let pairing_bytes = crypto
        .decrypt(encrypted_pairing)
        .map_err(|_| anyhow::anyhow!("Failed to decrypt pairing file"))?;

    let (mut handle, mut handshake) =
        open_cdtunnel_preferring_mdns(device_ip, mdns_ip, &pairing_bytes).await?;

    let team = sideloader
        .get_team()
        .await
        .map_err(|e| anyhow::anyhow!("get_team failed: {e}"))?;
    sideloader
        .get_dev_session()
        .ensure_device_registered(&team, device_name, device_udid, None)
        .await
        .map_err(|e| anyhow::anyhow!("ensure_device_registered failed: {e}"))?;

    info!("Signing {ipa_path}");
    let (signed_path, _special) = sideloader
        .sign_app(PathBuf::from(ipa_path), Some(team), false)
        .await
        .map_err(|e| anyhow::anyhow!("sign_app failed: {e}"))?;

    let real_bundle_id = {
        let p = signed_path.clone();
        tokio::task::spawn_blocking(move || read_bundle_id_from_app(&p))
            .await
            .map_err(|e| anyhow::anyhow!("Info.plist read panicked: {e}"))??
    };
    info!("Signed bundle id: {real_bundle_id}");

    let app_name = signed_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("signed_path has no filename"))?
        .to_string_lossy();
    let ipa_name = format!("{}.ipa", app_name.trim_end_matches(".app"));
    let afc_path = format!("PublicStaging/{}", ipa_name);

    info!("Zipping {} into {}", app_name, ipa_name);
    progress_cb(0, "zipping");
    let ipa_bytes = tokio::task::spawn_blocking({
        let signed_path = signed_path.clone();
        move || create_ipa(&signed_path)
    })
    .await
    .map_err(|e| anyhow::anyhow!("zip task panicked: {e}"))??;
    info!("IPA size: {} B", ipa_bytes.len());

    info!("Connecting AFC");
    let mut afc = AfcClient::connect_rsd(&mut handle, &mut handshake)
        .await
        .map_err(|e| anyhow::anyhow!("AFC connect failed: {e:?}"))?;

    afc.mk_dir("PublicStaging")
        .await
        .map_err(|e| anyhow::anyhow!("AFC create public staging failed: {e:?}"))?;

    let mut fh = afc
        .open(afc_path.as_str(), AfcFopenMode::WrOnly)
        .await
        .map_err(|e| anyhow::anyhow!("AFC open {afc_path} failed: {e:?}"))?;

    {
        use tokio::io::AsyncWriteExt;
        let total = ipa_bytes.len();
        let chunk_size = (total / 20).max(256 * 1024);
        let mut written = 0usize;
        let mut last_pct = 0u64;
        for chunk in ipa_bytes.chunks(chunk_size) {
            fh.write_all(chunk)
                .await
                .map_err(|e| anyhow::anyhow!("AFC write {afc_path} failed: {e}"))?;
            written += chunk.len();
            let pct = (written * 90 / total) as u64;
            if pct > last_pct {
                info!("AFC upload: {}%", pct);
                progress_cb(pct, "uploading");
                last_pct = pct;
            }
        }
    }

    fh.close()
        .await
        .map_err(|e| anyhow::anyhow!("AFC close {afc_path} failed: {e:?}"))?;
    drop(afc);

    info!("Connecting InstallationProxy");
    let mut instproxy = InstallationProxyClient::connect_rsd(&mut handle, &mut handshake)
        .await
        .map_err(|e| anyhow::anyhow!("InstallationProxy connect failed: {e:?}"))?;

    let mut opts = plist::Dictionary::new();
    opts.insert(
        "PackageType".to_string(),
        plist::Value::String("Developer".to_string()),
    );

    let progress_cb = std::sync::Arc::new(progress_cb);
    info!("Installing {}", afc_path);
    instproxy
        .install_with_callback(
            afc_path,
            Some(plist::Value::Dictionary(opts)),
            {
                let progress_cb = progress_cb.clone();
                move |(pct, _): (u64, ())| {
                    let cb = progress_cb.clone();
                    async move {
                        info!("Install: {}%", pct);
                        cb(90 + pct * 10 / 100, "installing");
                    }
                }
            },
            (),
        )
        .await
        .map_err(|e| anyhow::anyhow!("instproxy install failed: {e:?}"))?;

    info!("Install complete");
    Ok(real_bundle_id)
}

#[allow(clippy::too_many_arguments)]
pub async fn refresh_provisioning_profile(
    pool: &SqlitePool,
    crypto: &Crypto,
    apple_id: &str,
    encrypted_spd: &[u8],
    device_ip: &str,
    mdns_ip: Option<&str>,
    encrypted_pairing: &[u8],
    bundle_id: &str,
    anisette_url: &str,
) -> anyhow::Result<()> {
    let mut dev_session = get_dev_session(pool, apple_id, encrypted_spd, crypto, anisette_url).await?;

    let teams = dev_session
        .list_teams()
        .await
        .map_err(|e| anyhow::anyhow!("list_teams failed: {e}"))?;
    let team = teams
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No developer team available"))?;

    let app_ids = dev_session
        .list_app_ids(&team, None)
        .await
        .map_err(|e| anyhow::anyhow!("list_app_ids failed: {e}"))?;

    let app_id = app_ids
        .app_ids
        .into_iter()
        .find(|a| a.identifier == bundle_id)
        .ok_or_else(|| {
            anyhow::anyhow!("No App ID matching {bundle_id} on team {}", team.team_id)
        })?;

    info!("Fetching fresh provisioning profile for {bundle_id}");
    let profile = dev_session
        .download_team_provisioning_profile(&team, &app_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("download_team_provisioning_profile failed: {e}"))?;

    let pairing_bytes = crypto
        .decrypt(encrypted_pairing)
        .map_err(|_| anyhow::anyhow!("Failed to decrypt pairing file"))?;

    let (mut handle, mut handshake) =
        open_cdtunnel_preferring_mdns(device_ip, mdns_ip, &pairing_bytes).await?;

    info!("Connecting Misagent");
    let mut misagent = MisagentClient::connect_rsd(&mut handle, &mut handshake)
        .await
        .map_err(|e| anyhow::anyhow!("Misagent connect failed: {e:?}"))?;

    let profile_bytes: Vec<u8> = profile.encoded_profile.as_ref().to_vec();
    info!(
        "Installing provisioning profile {} ({} bytes)",
        profile.uuid,
        profile_bytes.len()
    );
    misagent
        .install(profile_bytes)
        .await
        .map_err(|e| anyhow::anyhow!("Misagent install failed: {e:?}"))?;

    info!("Refresh complete");
    Ok(())
}
