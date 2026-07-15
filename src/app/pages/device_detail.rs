use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::app::components::{confirm, round_up_duration};
use crate::app::{
    ChangeDeviceIp, DeleteApp, ExportPairing, InstallIpa, ReconcileApps, RefreshApp, RefreshDevice,
    ReimportPairing, SetRefreshEnabled, get_device_info, job_status, list_accounts, list_apps,
    probe_device,
};

/// Compute "expires in" label from installed_at / last_refreshed Unix timestamps.
fn expires_in_label(installed_at: Option<i64>, last_refreshed: Option<i64>) -> String {
    const CERT_LIFETIME_SECS: i64 = 7 * 86400;

    let Some(installed) = installed_at else {
        return "-".to_string();
    };

    #[cfg(target_arch = "wasm32")]
    let now_secs = (js_sys::Date::now() / 1000.0) as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let now_secs = chrono::Local::now().timestamp();

    let last_event = last_refreshed.map(|r| r.max(installed)).unwrap_or(installed);
    let expires_at = last_event + CERT_LIFETIME_SECS;
    let remaining = expires_at - now_secs;

    round_up_duration(remaining)
}

/// Live reachability label. `reachable_ms` comes from an active TCP probe run
/// just now; if that failed we fall back to the last mDNS sighting age as a
/// hint (mDNS never fires for VPN-only devices, so this is best-effort).
fn connection_status(reachable_ms: Option<u64>, mdns_seen_at: Option<i64>) -> (String, &'static str) {
    if let Some(ms) = reachable_ms {
        return (format!("Online ({ms}ms)"), "badge-online");
    }

    let Some(seen) = mdns_seen_at else {
        return ("Not reachable".to_string(), "badge-offline");
    };

    #[cfg(target_arch = "wasm32")]
    let now_secs = (js_sys::Date::now() / 1000.0) as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let now_secs = chrono::Local::now().timestamp();

    let age = now_secs - seen;
    if age < 3600 {
        (format!("Seen {}m ago", (age / 60).max(1)), "badge-stale")
    } else if age < 86400 {
        (format!("Seen {}h ago", age / 3600), "badge-offline")
    } else {
        (format!("Seen {}d ago", age / 86400), "badge-offline")
    }
}

#[component]
fn ConnectionBadge(device_id: Signal<String>) -> impl IntoView {
    let trigger = RwSignal::new(0u32);
    let probe = Resource::new(
        move || (device_id.get(), trigger.get()),
        |(id, _)| probe_device(id),
    );

    // Re-probe every 10s on the client so the badge stays live.
    Effect::new(move |_| {
        trigger.track();
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::closure::Closure;
            use wasm_bindgen::JsCast;
            let cb: Closure<dyn FnMut()> = Closure::once(move || {
                trigger.update(|n| *n += 1);
            });
            let _ = web_sys::window()
                .expect("window")
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    10_000,
                );
            cb.forget();
        }
    });

    view! {
        <Transition fallback=|| view! { <span class="badge badge-offline">"Pinging…"</span> }>
            {move || {
                probe
                    .get()
                    .map(|r| match r {
                        Ok(p) => {
                            let (label, cls) = connection_status(p.reachable_ms, p.mdns_seen_at);
                            view! { <span class=format!("badge {cls}")>{label}</span> }.into_any()
                        }
                        Err(_) => {
                            view! { <span class="badge badge-offline">"Unknown"</span> }.into_any()
                        }
                    })
            }}
        </Transition>
    }
}

#[cfg(target_arch = "wasm32")]
fn trigger_pairing_download(bytes_b64: &str) {
    use wasm_bindgen::JsCast;

    let data_url = format!("data:application/octet-stream;base64,{bytes_b64}");
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Ok(el) = document.create_element("a") else { return };
    let Ok(a) = el.dyn_into::<web_sys::HtmlAnchorElement>() else { return };
    a.set_href(&data_url);
    a.set_download("pairingFile.plist");
    if let Some(body) = document.body() {
        let _ = body.append_child(&a);
        a.click();
        let _ = body.remove_child(&a);
    }
}

// ── DeviceManagementCard ─────────────────────────────────────────────────────
// Extracted into its own component to keep DeviceDetail's view type shallow
// enough for the compiler's query-depth limit.

#[component]
fn DeviceManagementCard(
    device_id: Signal<String>,
    on_device_changed: Callback<()>,
    on_installed: Callback<()>,
) -> impl IntoView {
    let export_action = ServerAction::<ExportPairing>::new();
    let reimport_action = ServerAction::<ReimportPairing>::new();
    let change_ip_action = ServerAction::<ChangeDeviceIp>::new();

    let new_ip = RwSignal::new(String::new());
    let pairing_b64 = RwSignal::new(String::new());
    let pairing_file_status = RwSignal::new(String::new());
    let show_install = RwSignal::new(false);
    let show_change_ip = RwSignal::new(false);
    let show_reimport = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(Ok(())) = change_ip_action.value().get() {
            on_device_changed.run(());
            show_change_ip.set(false);
        }
    });

    Effect::new(move |_| {
        if let Some(Ok(ref _b64)) = export_action.value().get() {
            #[cfg(target_arch = "wasm32")]
            trigger_pairing_download(_b64);
        }
    });

    let did = device_id;

    view! {
        <section class="card">
            <h2>"Device Management"</h2>

            // Install IPA
            <div class="device-mgmt-row">
                <button
                    class="btn btn-secondary"
                    on:click=move |_| show_install.update(|v| *v = !*v)
                >
                    "Install IPA"
                </button>
            </div>

            // Export pairing
            <div class="device-mgmt-row">
                <ActionForm action=export_action>
                    <input type="hidden" name="device_id" value=move || did.get() />
                    <button type="submit" class="btn btn-secondary">
                        "Export Pairing"
                    </button>
                </ActionForm>
                {move || {
                    export_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(_) => {
                                view! { <span class="success">"Download started."</span> }
                                    .into_any()
                            }
                            Err(e) => {
                                view! { <span class="error">{e.to_string()}</span> }.into_any()
                            }
                        })
                }}
            </div>

            // Change IP
            <div class="device-mgmt-row">
                <button
                    class="btn btn-secondary"
                    on:click=move |_| show_change_ip.update(|v| *v = !*v)
                >
                    "Change IP"
                </button>
                {move || {
                    change_ip_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(()) => {
                                view! { <span class="success">"IP updated."</span> }.into_any()
                            }
                            Err(e) => {
                                view! { <span class="error">{e.to_string()}</span> }.into_any()
                            }
                        })
                }}
            </div>
            <Show when=move || show_change_ip.get()>
                <div class="device-mgmt-expand">
                    <div class="device-mgmt-inline">
                        <label class="form-field" style="flex:0 1 220px;min-width:140px">
                            "New IP"
                            <input
                                type="text"
                                prop:value=new_ip
                                on:input=move |e| new_ip.set(event_target_value(&e))
                                placeholder="e.g. 192.168.1.42"
                            />
                        </label>
                        <button
                            class="btn btn-secondary btn-mgmt-submit"
                            on:click=move |_| {
                                change_ip_action
                                    .dispatch(ChangeDeviceIp {
                                        device_id: did.get_untracked(),
                                        ip: new_ip.get_untracked(),
                                    });
                            }
                        >
                            "Save"
                        </button>
                    </div>
                </div>
            </Show>

            // Reimport pairing
            <div class="device-mgmt-row">
                <button
                    class="btn btn-secondary"
                    on:click=move |_| show_reimport.update(|v| *v = !*v)
                >
                    "Reimport Pairing"
                </button>
                {move || {
                    reimport_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(()) => {
                                view! { <span class="success">"Pairing updated."</span> }
                                    .into_any()
                            }
                            Err(e) => {
                                view! { <span class="error">{e.to_string()}</span> }.into_any()
                            }
                        })
                }}
            </div>
            <Show when=move || show_reimport.get()>
                <div class="device-mgmt-expand">
                    <div class="device-mgmt-inline">
                        <label class="form-field" style="flex:0 1 260px;min-width:160px">
                            "Pairing File (.plist)"
                            <input
                                type="file"
                                accept=".plist"
                                on:change=move |e| {
                                    #[cfg(target_arch = "wasm32")]
                                    read_file_b64(e, pairing_b64, pairing_file_status);
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        let _ = e;
                                    }
                                }
                            />
                            <small>{pairing_file_status}</small>
                        </label>
                        <button
                            class="btn btn-secondary btn-mgmt-submit"
                            prop:disabled=move || pairing_b64.get().is_empty()
                            on:click=move |_| {
                                let b64 = pairing_b64.get_untracked();
                                if b64.is_empty() {
                                    return;
                                }
                                reimport_action
                                    .dispatch(ReimportPairing {
                                        device_id: did.get_untracked(),
                                        pairing_blob_b64: b64,
                                    });
                                show_reimport.set(false);
                            }
                        >
                            "Import"
                        </button>
                    </div>
                </div>
            </Show>
        </section>

        // Install IPA card — revealed when the button above is toggled
        <Show when=move || show_install.get()>
            <section class="card">
                <h2>"Install IPA"</h2>
                <InstallForm
                    device_id=device_id
                    on_install=move || {
                        on_installed.run(());
                        show_install.set(false);
                    }
                />
            </section>
        </Show>
    }
}

// ── DeviceDetail ─────────────────────────────────────────────────────────────

#[component]
pub fn DeviceDetail() -> impl IntoView {
    let params = use_params_map();
    let device_id = move || {
        params
            .get()
            .get("id")
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let device = Resource::new(device_id, get_device_info);
    let apps = Resource::new(device_id, list_apps);

    let refresh_action = ServerAction::<RefreshApp>::new();
    let toggle_action = ServerAction::<SetRefreshEnabled>::new();
    let delete_app_action = ServerAction::<DeleteApp>::new();
    let reconcile_action = ServerAction::<ReconcileApps>::new();
    let refresh_all_action = ServerAction::<RefreshDevice>::new();

    Effect::new(move |_| {
        if refresh_action.version().get() > 0
            || toggle_action.version().get() > 0
            || delete_app_action.version().get() > 0
            || reconcile_action.version().get() > 0
            || refresh_all_action.version().get() > 0
        {
            apps.refetch();
        }
    });

    // Each major section is wrapped in .into_any() to erase its concrete type
    // and keep DeviceDetail's view tuple shallow (avoids compiler query-depth overflow).
    view! {
        <div class="page">
            {
                view! {
                    <Suspense fallback=|| view! { <p class="loading">"Loading device…"</p> }>
                        {move || {
                            device
                                .get()
                                .map(|r: Result<crate::app::DeviceInfo, _>| match r {
                                    Err(e) => {
                                        view! {
                                            <p class="error">"Device error: " {e.to_string()}</p>
                                        }
                                            .into_any()
                                    }
                                    Ok(dev) => {
                                        view! {
                                            <div class="page-header">
                                                <h1>{dev.name.clone()}</h1>
                                                <span class="badge badge-static">
                                                    {dev.ip.clone()}
                                                </span>
                                                <ConnectionBadge device_id=Signal::derive(
                                                    device_id,
                                                ) />
                                            </div>
                                            <div class="card">
                                                <table class="info-table">
                                                    <tbody>
                                                        <tr>
                                                            <th>"UDID"</th>
                                                            <td class="mono">{dev.udid.clone()}</td>
                                                        </tr>
                                                        <tr>
                                                            <th>"IP"</th>
                                                            <td>{dev.ip.clone()}</td>
                                                        </tr>
                                                        <tr>
                                                            <th>"Port"</th>
                                                            <td>{dev.port}</td>
                                                        </tr>
                                                        <tr>
                                                            <th>"Discovery"</th>
                                                            <td>{dev.discovery.clone()}</td>
                                                        </tr>
                                                    </tbody>
                                                </table>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                }
                    .into_any()
            }

            {
                view! {
                    <DeviceManagementCard
                        device_id=Signal::derive(device_id)
                        on_device_changed=Callback::new(move |_| device.refetch())
                        on_installed=Callback::new(move |_| apps.refetch())
                    />
                }
                    .into_any()
            }

            {
                view! {
                    <section class="card">
                <div class="page-header">
                    <h2>"Installed Apps"</h2>
                    <ActionForm action=reconcile_action>
                        <input type="hidden" name="device_id" value=device_id />
                        <button type="submit" class="btn btn-sm btn-secondary">
                            "Sync from device"
                        </button>
                    </ActionForm>
                    <ActionForm action=refresh_all_action>
                        <input type="hidden" name="device_id" value=device_id />
                        <button
                            type="submit"
                            class="btn btn-sm btn-primary"
                            prop:disabled=move || refresh_all_action.pending().get()
                        >
                            {move || {
                                if refresh_all_action.pending().get() {
                                    "Queuing..."
                                } else {
                                    "Refresh All"
                                }
                            }}
                        </button>
                    </ActionForm>
                </div>
                {move || {
                    reconcile_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(0) => {
                                view! {
                                    <div class="alert alert-success">"In sync with device."</div>
                                }
                                    .into_any()
                            }
                            Ok(n) => {
                                view! {
                                    <div class="alert alert-success">
                                        "Removed " {n} " stale entr"
                                        {if n == 1 { "y" } else { "ies" }} "."
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="alert alert-error">
                                        "Sync error: " {e.to_string()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
                {move || {
                    refresh_all_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(0) => {
                                view! { <div class="alert alert-success">"Nothing to refresh."</div> }
                                    .into_any()
                            }
                            Ok(n) => {
                                view! {
                                    <div class="alert alert-success">
                                        {n} " refresh job" {if n == 1 { "" } else { "s" }}
                                        " queued."
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="alert alert-error">
                                        "Refresh error: " {e.to_string()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
                <Suspense fallback=|| view! { <p class="loading">"Loading apps…"</p> }>
                    {move || {
                        apps
                            .get()
                            .map(|r: Result<Vec<crate::app::AppInfo>, _>| match r {
                                Err(e) => {
                                    view! { <p class="error">"Error: " {e.to_string()}</p> }
                                        .into_any()
                                }
                                Ok(app_list) if app_list.is_empty() => {
                                    view! {
                                        <p class="muted">"No apps installed on this device."</p>
                                    }
                                        .into_any()
                                }
                                Ok(app_list) => {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"App"</th>
                                                    <th>"Bundle ID"</th>
                                                    <th>"Expires In"</th>
                                                    <th>"Auto-Refresh"</th>
                                                    <th>"Actions"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {app_list
                                                    .into_iter()
                                                    .map(|app| {
                                                        let refresh_id = app.id.clone();
                                                        let toggle_id = app.id.clone();
                                                        let delete_id = app.id.clone();
                                                        let delete_msg = format!(
                                                            "Delete \"{}\" from this device?",
                                                            app.display_name,
                                                        );
                                                        let expiry_label = expires_in_label(
                                                            app.installed_at,
                                                            app.last_refreshed,
                                                        );
                                                        let expiry_class = if expiry_label
                                                            == "Expired"
                                                        {
                                                            "error"
                                                        } else {
                                                            ""
                                                        };
                                                        let has_ipa = app.has_ipa;
                                                        let refresh_enabled = app.refresh_enabled;
                                                        view! {
                                                            <tr>
                                                                <td>
                                                                    <strong>
                                                                        {app.display_name.clone()}
                                                                    </strong>
                                                                    {app
                                                                        .version
                                                                        .as_ref()
                                                                        .map(|v| {
                                                                            view! {
                                                                                <span class="version">
                                                                                    " v" {v.clone()}
                                                                                </span>
                                                                            }
                                                                        })}
                                                                </td>
                                                                <td class="mono">
                                                                    {app.bundle_id.clone()}
                                                                </td>
                                                                <td class=expiry_class>
                                                                    {expiry_label}
                                                                </td>
                                                                <td>
                                                                    <ActionForm action=toggle_action>
                                                                        <input
                                                                            type="hidden"
                                                                            name="app_id"
                                                                            value=toggle_id
                                                                        />
                                                                        <input
                                                                            type="hidden"
                                                                            name="enabled"
                                                                            value=if refresh_enabled {
                                                                                "false"
                                                                            } else {
                                                                                "true"
                                                                            }
                                                                        />
                                                                        <button
                                                                            type="submit"
                                                                            class=if refresh_enabled {
                                                                                "btn btn-sm btn-success"
                                                                            } else {
                                                                                "btn btn-sm btn-secondary"
                                                                            }
                                                                        >
                                                                            {if refresh_enabled {
                                                                                "ON"
                                                                            } else {
                                                                                "OFF"
                                                                            }}
                                                                        </button>
                                                                    </ActionForm>
                                                                </td>
                                                                <td class="actions">
                                                                    {if has_ipa {
                                                                        view! {
                                                                            <ActionForm
                                                                                action=refresh_action
                                                                            >
                                                                                <input
                                                                                    type="hidden"
                                                                                    name="app_id"
                                                                                    value=refresh_id
                                                                                />
                                                                                <button
                                                                                    type="submit"
                                                                                    class="btn btn-sm btn-primary"
                                                                                >
                                                                                    "Refresh"
                                                                                </button>
                                                                            </ActionForm>
                                                                        }
                                                                            .into_any()
                                                                    } else {
                                                                        view! {
                                                                            <span class="muted">
                                                                                "No IPA"
                                                                            </span>
                                                                        }
                                                                            .into_any()
                                                                    }}
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&delete_msg) {
                                                                            delete_app_action
                                                                                .dispatch(DeleteApp {
                                                                                    app_id: delete_id
                                                                                        .clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button
                                                                            type="submit"
                                                                            class="btn btn-sm btn-danger"
                                                                        >
                                                                            "Delete"
                                                                        </button>
                                                                    </form>
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>

                {move || {
                    refresh_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(job_id) => {
                                view! {
                                    <div>
                                        <div class="alert alert-success">"Refresh queued."</div>
                                        <JobProgress
                                            job_id=job_id
                                            on_done=Callback::new(move |_| apps.refetch())
                                        />
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="alert alert-error">
                                        "Refresh error: " {e.to_string()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </section>
                }
                    .into_any()
            }
        </div>
    }
}

// ── InstallForm ───────────────────────────────────────────────────────────────

#[component]
fn InstallForm(device_id: Signal<String>, #[prop(into)] on_install: Callback<()>) -> impl IntoView {
    let accounts = Resource::new(|| (), |_| list_accounts());
    let install_action = ServerAction::<InstallIpa>::new();

    let ipa_b64 = RwSignal::new(String::new());
    let file_status = RwSignal::new(String::new());
    let account_id = RwSignal::new(String::new());
    let store = RwSignal::new(true);
    let active_job = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        if let Some(Ok(ref id)) = install_action.value().get() {
            active_job.set(Some(id.clone()));
        }
    });

    let can_submit = move || !ipa_b64.get().is_empty() && !account_id.get().is_empty();

    view! {
        <div>
            <div class="form-row">
                <label class="form-field">
                    "Account"
                    <Suspense fallback=|| {
                        view! {
                            <select disabled>
                                <option>"Loading..."</option>
                            </select>
                        }
                    }>
                        {move || {
                            accounts
                                .get()
                                .map(|r| {
                                    let accs = r.unwrap_or_default();
                                    view! {
                                        <select on:change=move |e| {
                                            account_id.set(event_target_value(&e))
                                        }>
                                            <option value="">"- Select Account -"</option>
                                            {accs
                                                .into_iter()
                                                .map(|a| {
                                                    let label = match &a.team_name {
                                                        Some(t) => {
                                                            format!("{} ({})", a.apple_id, t)
                                                        }
                                                        None => a.apple_id.clone(),
                                                    };
                                                    view! { <option value=a.id>{label}</option> }
                                                })
                                                .collect_view()}
                                        </select>
                                    }
                                })
                        }}
                    </Suspense>
                </label>
                <label class="form-field">
                    "IPA File"
                    <input
                        type="file"
                        accept=".ipa"
                        on:change=move |e| {
                            #[cfg(target_arch = "wasm32")]
                            read_file_b64(e, ipa_b64, file_status);
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let _ = e;
                            }
                        }
                    /> <small>{file_status}</small>
                </label>
                <label class="form-field checkbox">
                    <input
                        type="checkbox"
                        prop:checked=store
                        on:change=move |e| store.set(event_target_checked(&e))
                    />
                    " Store IPA for auto-refresh"
                </label>
            </div>
            <button
                class="btn btn-primary"
                prop:disabled=move || !can_submit()
                on:click=move |_| {
                    if !can_submit() {
                        return;
                    }
                    install_action
                        .dispatch(InstallIpa {
                            device_id: device_id.get_untracked(),
                            account_id: account_id.get_untracked(),
                            ipa_bytes_b64: ipa_b64.get_untracked(),
                            store: store.get_untracked(),
                        });
                }
            >
                "Install"
            </button>
        </div>

        {move || {
            install_action
                .value()
                .get()
                .map(|r| match r {
                    Err(e) => {
                        view! {
                            <div class="alert alert-error">"Install error: " {e.to_string()}</div>
                        }
                            .into_any()
                    }
                    Ok(_) => {
                        view! { <div class="alert alert-success">"Install queued."</div> }
                            .into_any()
                    }
                })
        }}

        {move || {
            active_job
                .get()
                .map(|id| {
                    view! {
                        <JobProgress job_id=id on_done=Callback::new(move |_| on_install.run(())) />
                    }
                })
        }}
    }
}

// ── File reader helper ────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn read_file_b64(e: leptos::ev::Event, out: RwSignal<String>, status: RwSignal<String>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(input) = e
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };

    let Some(files) = input.files() else { return };
    let Some(file) = files.get(0) else { return };

    status.set(format!("Reading {}...", file.name()));
    let reader = web_sys::FileReader::new().expect("FileReader");
    let r2 = reader.clone();
    let cb: Closure<dyn FnMut()> = Closure::once(move || {
        if let Ok(v) = r2.result() {
            if let Some(data_url) = v.as_string() {
                if let Some(b64) = data_url.split(',').nth(1) {
                    out.set(b64.to_string());
                    status.set("File loaded.".to_string());
                }
            }
        }
    });
    reader.set_onload(Some(cb.as_ref().unchecked_ref()));
    reader.read_as_data_url(&file).expect("read_as_data_url");
    cb.forget();
}

// ── JobProgress ───────────────────────────────────────────────────────────────

#[component]
fn JobProgress(
    job_id: String,
    #[prop(optional, into)] on_done: Option<Callback<()>>,
) -> impl IntoView {
    let job_id = StoredValue::new(job_id);
    let trigger = RwSignal::new(0u32);
    let job = Resource::new(
        move || (job_id.get_value(), trigger.get()),
        |(id, _)| job_status(id),
    );

    let is_done = move || {
        job.get()
            .and_then(|r| r.ok())
            .map(|j| j.status == "done" || j.status == "failed")
            .unwrap_or(false)
    };

    Effect::new(move |_| {
        if !is_done() {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;
                let cb: Closure<dyn FnMut()> = Closure::once(move || {
                    trigger.update(|n| *n += 1);
                });
                let _ = web_sys::window()
                    .expect("window")
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.as_ref().unchecked_ref(),
                        2_000,
                    );
                cb.forget();
            }
        }
    });

    let fired = StoredValue::new(false);
    Effect::new(move |_| {
        if is_done() && !fired.get_value() {
            fired.set_value(true);
            if let Some(cb) = on_done {
                cb.run(());
            }
        }
    });

    view! {
        <div class="job-progress">
            <Suspense fallback=|| view! { <span class="loading">"..."</span> }>
                {move || {
                    job
                        .get()
                        .map(|r| match r {
                            Err(e) => {
                                view! { <span class="error">"Job error: " {e.to_string()}</span> }
                                    .into_any()
                            }
                            Ok(j) => {
                                let badge_class = match j.status.as_str() {
                                    "done" => "job-badge job-done",
                                    "failed" => "job-badge job-failed",
                                    "running" => "job-badge job-running",
                                    _ => "job-badge job-queued",
                                };
                                let is_running = j.status == "running";
                                let progress = j.progress;
                                let stage = j.stage.clone();
                                view! {
                                    <span class=badge_class>{j.status.clone()}</span>
                                    {is_running
                                        .then(|| {
                                            stage
                                                .map(|s| {
                                                    view! {
                                                        <span class="job-stage">" - " {s}</span>
                                                    }
                                                })
                                        })}
                                    {is_running
                                        .then(|| {
                                            view! {
                                                <div class="job-progress-bar-wrap">
                                                    <progress
                                                        max="100"
                                                        value=progress
                                                        class="job-progress-bar"
                                                    ></progress>
                                                    <span class="job-progress-pct">
                                                        {progress} "%"
                                                    </span>
                                                </div>
                                            }
                                        })}
                                    {j
                                        .error
                                        .map(|e| view! { <span class="error">" - " {e}</span> })}
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}
