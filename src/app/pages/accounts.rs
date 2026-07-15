use leptos::prelude::*;

use crate::app::components::{confirm, round_up_duration};
use crate::app::{
    begin_login, complete_login, export_livecontainer_cert, list_account_app_ids, list_accounts,
    AppIdsResult, DeleteAccount, LcCertExport, RevokeAllCerts,
};

#[component]
pub fn Accounts() -> impl IntoView {
    let accounts = Resource::new(|| (), |_| list_accounts());
    let delete_action = ServerAction::<DeleteAccount>::new();
    let revoke_action = ServerAction::<RevokeAllCerts>::new();

    let lc_exporting = RwSignal::new(false);
    let lc_export_result = RwSignal::<Option<Result<LcCertExport, String>>>::new(None);

    let app_ids_loading = RwSignal::new(false);
    let app_ids_result = RwSignal::<Option<Result<AppIdsResult, String>>>::new(None);
    let app_ids_label = RwSignal::new(String::new());

    Effect::new(move |_| {
        if delete_action.version().get() > 0 {
            accounts.refetch();
        }
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Apple Accounts"</h1>
            </div>

            {move || {
                revoke_action
                    .value()
                    .get()
                    .map(|r| match r {
                        Ok(n) => {
                            view! {
                                <div class="alert alert-success">
                                    "Revoked " {n}
                                    " iOS development certificate(s). You can now install apps again."
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => {
                            view! {
                                <div class="alert alert-error">
                                    "Revoke failed: " {e.to_string()}
                                </div>
                            }
                                .into_any()
                        }
                    })
            }}

            <section class="card">
                <h2>"Add Apple ID"</h2>
                <AddAccountForm on_success=move || accounts.refetch() />
            </section>

            <section class="card">
                <h2>"Accounts"</h2>
                <Suspense fallback=|| {
                    view! { <p class="loading">"Loading..."</p> }
                }>
                    {move || {
                        accounts
                            .get()
                            .map(|r| match r {
                                Err(e) => {
                                    view! { <p class="error">"Error: " {e.to_string()}</p> }
                                        .into_any()
                                }
                                Ok(accs) if accs.is_empty() => {
                                    view! { <p class="muted">"No accounts added yet."</p> }
                                        .into_any()
                                }
                                Ok(accs) => {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"Apple ID"</th>
                                                    <th>"Team"</th>
                                                    <th>"Actions"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {accs
                                                    .into_iter()
                                                    .map(|a| {
                                                        let delete_id = a.id.clone();
                                                        let revoke_id = a.id.clone();
                                                        let export_id = a.id.clone();
                                                        let appids_id = a.id.clone();
                                                        let appids_email = a.apple_id.clone();
                                                        let delete_msg = format!(
                                                            "Remove account \"{}\" from JAS? Installed apps signed with this account will keep running until their certs expire.",
                                                            a.apple_id,
                                                        );
                                                        let revoke_msg = format!(
                                                            "Revoke ALL iOS development certificates for \"{}\"? Apps signed with the revoked cert will stop working until you reinstall them.",
                                                            a.apple_id,
                                                        );
                                                        view! {
                                                            <tr>
                                                                <td>{a.apple_id.clone()}</td>
                                                                <td>
                                                                    {a.team_name.as_deref().unwrap_or("-").to_string()}
                                                                    {a
                                                                        .team_id
                                                                        .as_ref()
                                                                        .map(|t| {
                                                                            view! { <span class="muted">" (" {t.clone()} ")"</span> }
                                                                        })}
                                                                </td>
                                                                <td class="actions">
                                                                    <button
                                                                        type="button"
                                                                        class="btn btn-sm btn-secondary"
                                                                        prop:disabled=app_ids_loading
                                                                        on:click=move |_| {
                                                                            app_ids_loading.set(true);
                                                                            app_ids_result.set(None);
                                                                            app_ids_label.set(appids_email.clone());
                                                                            let id = appids_id.clone();
                                                                            leptos::task::spawn_local(async move {
                                                                                let result = list_account_app_ids(id)
                                                                                    .await
                                                                                    .map_err(|e| e.to_string());
                                                                                app_ids_result.set(Some(result));
                                                                                app_ids_loading.set(false);
                                                                            });
                                                                        }
                                                                    >
                                                                        {move || {
                                                                            if app_ids_loading.get() {
                                                                                "Loading..."
                                                                            } else {
                                                                                "App IDs"
                                                                            }
                                                                        }}
                                                                    </button>
                                                                    <button
                                                                        type="button"
                                                                        class="btn btn-sm btn-secondary"
                                                                        prop:disabled=lc_exporting
                                                                        on:click=move |_| {
                                                                            lc_exporting.set(true);
                                                                            lc_export_result.set(None);
                                                                            let id = export_id.clone();
                                                                            leptos::task::spawn_local(async move {
                                                                                let result = export_livecontainer_cert(id)
                                                                                    .await
                                                                                    .map_err(|e| e.to_string());
                                                                                lc_export_result.set(Some(result));
                                                                                lc_exporting.set(false);
                                                                            });
                                                                        }
                                                                    >
                                                                        {move || {
                                                                            if lc_exporting.get() {
                                                                                "Exporting..."
                                                                            } else {
                                                                                "Export LC Cert"
                                                                            }
                                                                        }}
                                                                    </button>
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&revoke_msg) {
                                                                            revoke_action
                                                                                .dispatch(RevokeAllCerts {
                                                                                    account_id: revoke_id.clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button type="submit" class="btn btn-sm btn-warning">
                                                                            "Revoke Certs"
                                                                        </button>
                                                                    </form>
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&delete_msg) {
                                                                            delete_action
                                                                                .dispatch(DeleteAccount {
                                                                                    id: delete_id.clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button type="submit" class="btn btn-sm btn-danger">
                                                                            "Remove"
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
            </section>

            {move || {
                app_ids_result.get().map(|result| match result {
                    Err(e) => {
                        view! {
                            <div class="alert alert-error">"App ID list failed: " {e}</div>
                        }
                            .into_any()
                    }
                    Ok(data) => {
                        let label = app_ids_label.get();
                        let summary = match (data.max_quantity, data.available_quantity) {
                            (Some(max), Some(avail)) => {
                                let used = max.saturating_sub(avail.max(0) as u64);
                                format!("{used} / {max} slots used")
                            }
                            _ => format!("{} registered", data.entries.len()),
                        };
                        view! {
                            <section class="card">
                                <h2>"App IDs — " {label}</h2>
                                <p class="muted" style="margin-bottom:12px">{summary}</p>
                                {if data.entries.is_empty() {
                                    view! {
                                        <p class="muted">"No App IDs registered."</p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"Name"</th>
                                                    <th>"Identifier"</th>
                                                    <th>"Expires In"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {data
                                                    .entries
                                                    .into_iter()
                                                    .map(|e| {
                                                        #[cfg(target_arch = "wasm32")]
                                                        let now_secs = (js_sys::Date::now() / 1000.0)
                                                            as i64;
                                                        #[cfg(not(target_arch = "wasm32"))]
                                                        let now_secs = chrono::Local::now().timestamp();

                                                        let expiry_label = e
                                                            .expiration_date
                                                            .map(|ts| round_up_duration(ts - now_secs))
                                                            .unwrap_or_else(|| "-".to_string());
                                                        let expiry_class = if expiry_label
                                                            == "Expired"
                                                        {
                                                            "error"
                                                        } else {
                                                            ""
                                                        };
                                                        view! {
                                                            <tr>
                                                                <td>{e.name}</td>
                                                                <td class="mono">
                                                                    {e.identifier}
                                                                </td>
                                                                <td class=expiry_class>
                                                                    {expiry_label}
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    }
                                        .into_any()
                                }}
                            </section>
                        }
                            .into_any()
                    }
                })
            }}

            {move || {
                lc_export_result.get().map(|result| match result {
                    Err(e) => {
                        view! {
                            <div class="alert alert-error">"Certificate export failed: " {e}</div>
                        }
                            .into_any()
                    }
                    Ok(cert) => {
                        let download_href = format!(
                            "data:application/octet-stream;base64,{}",
                            cert.p12_b64,
                        );
                        // livecontainer://certificate?cert=<BASE64>&password=<PW>
                        // Base64 uses +, /, = which must be percent-encoded in a query string.
                        let lc_cert_encoded = cert.p12_b64
                            .replace('+', "%2B")
                            .replace('/', "%2F")
                            .replace('=', "%3D");
                        let lc_href = format!(
                            "livecontainer://certificate?cert={}&password={}",
                            lc_cert_encoded,
                            cert.password,
                        );
                        view! {
                            <section class="card lc-cert-export">
                                <h2>"LiveContainer Certificate"</h2>
                                <p class="muted">
                                    "Use \"Add to LiveContainer\" to import directly, or download "
                                    "the file and rename it from "
                                    <code>".p"</code> " to " <code>".p12"</code>
                                    " in the Files app before importing manually."
                                </p>
                                <p>"Password: " <code>{cert.password.clone()}</code></p>
                                <div class="lc-cert-actions">
                                    <a class="btn btn-primary" href=lc_href>
                                        "Add to LiveContainer"
                                    </a>
                                    <a
                                        class="btn btn-secondary"
                                        href=download_href
                                        download="ALTCertificate.p"
                                    >
                                        "Download ALTCertificate.p"
                                    </a>
                                </div>
                            </section>
                        }
                            .into_any()
                    }
                })
            }}
        </div>
    }
}

#[component]
fn AddAccountForm(#[prop(into)] on_success: Callback<()>) -> impl IntoView {
    let apple_id = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let session_key = RwSignal::<Option<String>>::new(None);
    let tfa_code = RwSignal::new(String::new());
    let error_msg = RwSignal::<Option<String>>::new(None);
    let pending = RwSignal::new(false);

    let on_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let id = apple_id.get();
        let pw = password.get();
        pending.set(true);
        error_msg.set(None);

        leptos::task::spawn_local(async move {
            match begin_login(id, pw).await {
                Ok((key, needs_2fa)) => {
                    if needs_2fa {
                        session_key.set(Some(key));
                    } else {
                        on_success.run(());
                    }
                }
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                }
            }
            pending.set(false);
        });
    };

    let on_2fa_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let key = match session_key.get() {
            Some(k) => k,
            None => return,
        };
        let code = tfa_code.get();
        pending.set(true);
        error_msg.set(None);

        leptos::task::spawn_local(async move {
            match complete_login(key, code).await {
                Ok(()) => {
                    session_key.set(None);
                    on_success.run(());
                }
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                }
            }
            pending.set(false);
        });
    };

    view! {
        {move || {
            if session_key.get().is_none() {
                view! {
                    <form on:submit=on_submit>
                        <div class="form-row">
                            <label class="form-field">
                                "Apple ID"
                                <input
                                    type="email"
                                    required
                                    placeholder="you@example.com"
                                    prop:value=apple_id
                                    on:input=move |e| apple_id.set(event_target_value(&e))
                                />
                            </label>
                            <label class="form-field">
                                "Password"
                                <input
                                    type="password"
                                    required
                                    placeholder="•••••••••"
                                    prop:value=password
                                    on:input=move |e| password.set(event_target_value(&e))
                                />
                            </label>
                        </div>
                        <p class="hint">"Your password is used once and never stored."</p>
                        <button type="submit" class="btn btn-primary" prop:disabled=pending>
                            {move || if pending.get() { "Signing in..." } else { "Sign In" }}
                        </button>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <form on:submit=on_2fa_submit>
                        <p>"A two-factor code was sent to your trusted device."</p>
                        <div class="form-row">
                            <label class="form-field">
                                "2FA Code"
                                <input
                                    type="text"
                                    required
                                    placeholder="000000"
                                    maxlength="6"
                                    prop:value=tfa_code
                                    on:input=move |e| tfa_code.set(event_target_value(&e))
                                />
                            </label>
                        </div>
                        <button type="submit" class="btn btn-primary" prop:disabled=pending>
                            {move || if pending.get() { "Verifying..." } else { "Submit Code" }}
                        </button>
                    </form>
                }
                    .into_any()
            }
        }}

        {move || error_msg.get().map(|e| view! { <p class="error">{e}</p> })}
    }
}
