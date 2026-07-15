# JAS

**Jackson's App Sensei**: a self-hosted alternative to AltServer that runs on a
server and exposes a web portal for installing and refreshing iOS apps over a
local network or VPN.

---

## Overview

JAS is a single Leptos fullstack binary that:

- Manages iOS devices reachable by IP (VPN tunnel peers or
mDNS-discovered LAN devices)
- Signs and installs IPA files onto those devices using an Apple ID, via the
`isideload` crate
- Refreshes installed apps before their 7-day developer certificate expires
- Exposes a Leptos web portal for all of the above, using server functions for
all backend communication

---

## Features

- Leptos fullstack SSR portal - no separate frontend build step, no REST API layer,
NO JAVASCRIPT >:(
- SQLite-backed device registry, app tracking, certificate management, and job history
- Static IP registration for VPN peers, with mDNS as a fast LAN-IP for those
same devices
- RSD transport for iOS 17+ devices over VPN
- Automatic refresh scheduler - re-signs stored IPAs within 3 days of
certificate expiry, with a misagent-only fast path before falling back to a full
sign + install
- Ephemeral install mode for IPAs you don't want stored on the server
- Live job progress via reactive server function polling
- LiveContainer support: export the current iOS development certificate as a
  PKCS#12 file directly from the Accounts page
- Works in sleep mode

---

## Non-Goals

- JAS does not manage WireGuard: it only reads the IPs you assign to devices.
- JAS does not talk to `usbmuxd`. Every device is reached over TCP via its
RPPairing file; lockdown-only devices are out of scope.
- JAS does not auto-register devices from mDNS. Discovery is a LAN-IP hint
for already-registered devices, never a registration path.
- JAS does not implement AltStore-style "sources" or a plugin ecosystem.

---

## FAQ

- Why? Servers are always on. I don’t need to turn on LocalDevVPN, I’m
always on my WireGuard instance. I always forget to refresh myself,
servers don’t forget. Servers can’t expire, making me need to reinstall
everything. Servers don't take an app slot like a store app does.

- What's with the name? Naming things is hard, so the original name was
Jackson's App Server. A good friend suggested App Sensei since 'server'
is way overused in this ecosystem, but it was too late to change the
acronym from JAS.

- What about feature 'x'? This is very much a project that I designed
for me to use. People are welcome to PR features they want, but I designed
this for my use-case in mind.

- Should I use it? Idk, I'm not an expert at Apple servers, I'm probably
doing something dumb.

---

## Building

Prerequisites:

- Rust
- `cargo-leptos`: `cargo install --locked cargo-leptos`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- OpenSSL on the host, or enable the `vendored-openssl` feature in the
`isideload` dependency

```bash
cargo leptos build --release
```

Output: a single binary at `target/release/jas` and a `target/site/` directory of
static assets. Both must be present at runtime (`LEPTOS_SITE_ROOT` points the
binary to the assets, configurable in `Cargo.toml` under `[package.metadata.leptos]`).

To run on Linux (Place /target/site next to the jas binary):

```bash
JAS_SECRET_KEY=<32-byte-hex> ./jas
```

The `JAS_SECRET_KEY` flag is optional but protects your data locally. You can build the project yourself or download a prebuilt binary from [GitHub Actions](https://github.com/jkcoxson/jas/actions).

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    JAS Binary                       │
│                                                     │
│  ┌──────────────────────────────────────────────┐   │
│  │          Leptos + leptos_axum                │   │
│  │                                              │   │
│  │  Server Functions -> backend logic           │   │
│  │  SSR components   -> rendered HTML           │   │
│  │  WASM hydration   -> interactive frontend    │   │
│  └──────────────────┬───────────────────────────┘   │
│                     │                               │
│  ┌──────────────────▼───────────────────────────┐   │
│  │              SQLite (sqlx)                   │   │
│  │  devices | apps | accounts | jobs            │   │
│  └──────────────────┬───────────────────────────┘   │
│                     │                               │
│  ┌──────────────────▼───────────────────────────┐   │
│  │   isideload  +  idevice                      │   │
│  │   RSD-over-TCP transport for IP-reachable    │   │
│  │   devices; Apple ID auth + IPA signing       │   │
│  └──────────────────────────────────────────────┘   │
│                     │                               │
│  ┌──────────────────▼───────────────────────────┐   │
│  │         Background Task Pool (tokio)         │   │
│  │   refresh scheduler | install queue          │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
          │ TCP (VPN IP or LAN IP + RSD)
          ▼
     iOS Devices
```

---

## Key Dependencies

| Crate | Purpose |
| --- | --- |
| `leptos` | Fullstack reactive UI framework |
| `leptos_axum` | Axum integration, server function routing |
| `axum` | Underlying HTTP server (managed by leptos_axum) |
| `sqlx` | Async SQLite with compile-time query checking |
| `isideload` | Apple ID auth, cert provisioning, IPA signing; |
| `idevice` | Device communication (lockdownd, install proxy, RSD) |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `tracing` + `tracing-subscriber` | Structured logging |
| `mdns-sd` | mDNS device discovery |
| `uuid` | Stable device/job IDs |
| `chrono` | Certificate expiry tracking |
| `aes-gcm` | Encryption for stored secrets |

---

## Device Model

Devices are identified by a **stable UUID** generated at first registration.
Each device record holds:

```sql
CREATE TABLE devices (
    id            TEXT NOT NULL PRIMARY KEY,  -- UUID
    name          TEXT NOT NULL,        -- auto-read from lockdown at registration
    udid          TEXT NOT NULL UNIQUE,
    ip            TEXT NOT NULL,        -- VPN IP or LAN IP (source of truth)
    port          INTEGER NOT NULL DEFAULT 49152,
    pairing_blob  BLOB NOT NULL,        -- encrypted RPPairing file
    last_seen     INTEGER,
    discovery     TEXT NOT NULL DEFAULT 'static',
    mdns_ip       TEXT,                 -- last LAN IP resolved via mDNS, if any
    mdns_seen_at  INTEGER
);
```

Devices are registered manually via the web portal with an explicit IP,
typically a VPN peer address.
The IP a user provides is the permanent source of truth.

The mDNS task (`_remotepairing._tcp.local.`) is a **LAN hint**.
It browses the local network and, for each resolved peer, verifies the peer
against each registered device's `alt_irk` from the pairing file.
On a match it stamps `mdns_ip` / `mdns_seen_at` on that device.
At connect time the transport tries `mdns_ip` first for a fast LAN path, then
falls back to the manual IP.

Every transport opens an **RSD (Remote Service Discovery)** tunnel over TCP to
the device's IP using the trusted RPPairing file the user uploads at
registration. This was chosen for VPN allowance on iOS 26.4+, doesn't require
a heartbeat (it's complicated), and is reliable in sleep mode.

---

## App & Account Model

```sql
CREATE TABLE apple_accounts (
    id           TEXT NOT NULL PRIMARY KEY,
    apple_id     TEXT NOT NULL UNIQUE,
    -- The password is NOT stored; only the encrypted SPD plist (containing the
    -- GsIdmsToken needed to mint xcode.auth tokens) lives here. Anisette state
    -- and the cached xcode.auth token are scoped per Apple ID in sideload_storage.
    session_blob BLOB NOT NULL,
    team_id      TEXT,
    team_name    TEXT
);

CREATE TABLE apps (
    id              TEXT NOT NULL PRIMARY KEY,
    device_id       TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    bundle_id       TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    version         TEXT,
    ipa_path        TEXT,              -- NULL if not stored locally
    account_id      TEXT REFERENCES apple_accounts(id) ON DELETE SET NULL,
    installed_at    INTEGER,           -- NULL while an install job is in flight
    last_refreshed  INTEGER,
    refresh_enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(device_id, bundle_id)
);

CREATE TABLE jobs (
    id          TEXT NOT NULL PRIMARY KEY,
    app_id      TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,                       -- 'install' | 'refresh'
    status      TEXT NOT NULL DEFAULT 'queued',      -- 'queued' | 'running' | 'done' | 'failed'
    error       TEXT,
    started_at  INTEGER,
    finished_at INTEGER,
    progress    INTEGER NOT NULL DEFAULT 0,
    stage       TEXT
);

-- Generic key/value store. Used by isideload for per-Apple-ID anisette state
-- and cached xcode.auth tokens, and by JAS for live-editable settings
-- (e.g. "_server/anisette_url").
CREATE TABLE sideload_storage (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
```

Certificate state is owned by `isideload` and persisted in `sideload_storage`.
JAS does not track certs in its own table, refresh decisions key off
`installed_at` / `last_refreshed` and Apple's fixed 7-day developer-cert lifetime.

---

## Signing & Installation Flow

Signing is delegated entirely to `isideload`:

1. **Account auth**: `AppleAccount` authenticates with Apple ID credentials via
Apple's private developer APIs. The password is used once; only the resulting
SPD (session token) and the minted `xcode.auth` app token are persisted (both encrypted).

2. **Certificate provisioning**: `DeveloperSession` fetches or creates the
device-scoped development certificate and provisioning profile each install;
`isideload` caches them in `sideload_storage` and reuses them until expiry.

3. **Transport**: an RSD tunnel is opened over TCP via the device's RPPairing
file to reach `installation_proxy`, `afc`, and `misagent`.

4. **Signing + install**: `sideload` re-signs the IPA, AFC-uploads it to
`PublicStaging/`, and asks `installation_proxy` to install.
Progress is written to the `jobs` table (`progress` / `stage`) and surfaced live
in the portal.

### IPA Storage

IPAs can be:

- **Stored** in a configurable directory on the server (`ipa_dir` in config).
The `ipa_path` column points to the file. Required for automatic refresh without
re-uploading.

- **Ephemeral**: uploaded, installed, then discarded. `ipa_path` stays NULL;
the app is tracked but the scheduler skips it.

---

## Refresh Scheduler

A `tokio` background task wakes on a configurable interval (default: every 2
hours) and queries apps whose certificate is expected to expire within
`refresh_window_days` (default: 3). Expiry is computed from the last
install/refresh timestamp plus the fixed 7-day Apple developer-cert lifetime:

```sql
SELECT *
FROM apps
WHERE refresh_enabled = 1
  AND ipa_path IS NOT NULL
  AND account_id IS NOT NULL
  AND installed_at IS NOT NULL
  AND COALESCE(last_refreshed, installed_at)
        < unixepoch() - ((7 - :window_days) * 86400)
```

Matching apps are first verified against the device's `installation_proxy`
listing. Anything missing on the device is dropped from the DB.
Survivors are enqueued into a `tokio::sync::mpsc` channel.
A worker pool (configurable, default: 2) drains the channel and, for each job,
first tries the fast refresh path (download a fresh provisioning profile and
push it to `misagent`); if that fails, it falls back to a full sign + install.
Results are written back to the `jobs` table.

---

## Web Portal

### Pages

**`/` — Dashboard**: Device grid with per-card app count, next expiry, expiring-soon
/ expired counts, plus quick-action buttons (Manage Apps, Refresh All).

**`/devices` — Device Management**: List of all registered devices. Add-device
form (IP + RPPairing file upload). The device's last LAN IP resolved via
mDNS appears as a sub-line under its manual IP. Delete cascades to apps.

**`/devices/:id` — Device Detail**: Installed app list with refresh toggle,
bundle ID, version, last-refreshed time. IPA upload form triggers a sign +
install job.
Live job progress shown inline. "Sync from device" reconciles the DB against
`installation_proxy` and prunes apps that no longer exist on the device.

**`/accounts` — Apple ID Management**: Add Apple ID via a two-step
`begin_login` / `complete_login` flow. The password is used once during the
initial GrandSlam handshake and never stored; only the SPD session and the
minted xcode.auth token are persisted, encrypted. When Apple requires 2FA,
the UI surfaces an inline code prompt that resumes the same server-side login.
Each row lists `apple_id`, `team_name (team_id)`, and three actions:

- **App IDs** — fetches and displays all registered App IDs for the team
  (name, bundle identifier, expiry date) with the slot count used vs. the
  team maximum.
- **Export LC Cert** — provisions or retrieves the current iOS development
  certificate and exports it as a PKCS#12 file (`.p` extension; rename to
  `.p12` in the Files app before importing into LiveContainer). The file
  password is shown alongside the download link.
- **Revoke Certs** — revokes every iOS development certificate on the team
  via Apple's developer API; apps signed with the revoked cert stop launching
  until reinstalled.
- **Remove** — deletes the account row from JAS; installed apps keep running
  until their existing cert expires.

**`/settings` — Server Config**: Read-only view of `jas.toml` values (bind
address, log level, storage paths, scheduler tuning, mDNS). Also contains a
live-editable **Anisette Server** field — changes take effect immediately and
persist across restarts.

---

## Configuration

Loaded from `jas.toml` (path overridable via `JAS_CONFIG` env var):

```toml
[server]
bind = "0.0.0.0:3000"
log_level = "info"

[storage]
database_path = "/var/lib/jas/jas.db"
ipa_dir = "/var/lib/jas/ipas"

[scheduler]
interval_hours = 2
refresh_window_days = 3
worker_threads = 2

[security]
# Set via JAS_SECRET_KEY env var in production.
# Used to encrypt stored private keys and Apple account session tokens.
secret_key = ""   # 32-byte hex string

[discovery]
mdns_enabled = true
mdns_interface = ""   # empty = all interfaces
```

### Environment variables

| Variable | Description |
| --- | --- |
| `JAS_SECRET_KEY` | 32-byte hex encryption key (recommended over `jas.toml`) |
| `JAS_CONFIG` | Path to config file (default: `jas.toml`) |

---

## Security Considerations

- **Private keys and session tokens** are encrypted at rest with AES-256-GCM.
Raw values never touch SQLite unencrypted.
- **Apple ID passwords** are not stored after the initial auth handshake.
Only the session token is persisted.
- **The web portal has no built-in authentication.** It is intended to be
deployed behind a NAT.
- Jackson has no clue what he's doing, use at your own risk

---

## License

Non-commercial usage and modification of JAS-specific code is licensed
under the MIT license.

Commercial usage requires explicit, written permission from Jackson Coxson.

All dependencies remain under their original licenses.
