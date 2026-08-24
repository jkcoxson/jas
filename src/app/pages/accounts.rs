use leptos::prelude::*;

use crate::app::components::{confirm, round_up_duration};
use crate::app::{
    begin_login, export_livecontainer_cert, list_account_app_ids, list_accounts, poll_login,
    submit_two_factor, AppIdsResult, DeleteAccount, LcCertExport, LoginStep, RevokeAllCerts,
    TrustedNumberInfo, TwoFactorAction, TwoFactorPrompt,
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
    let prompt = RwSignal::<Option<TwoFactorPrompt>>::new(None);
    let tfa_code = RwSignal::new(String::new());
    let error_msg = RwSignal::<Option<String>>::new(None);
    let pending = RwSignal::new(false);

    // Advance the login, polling through any `Pending` steps, until it either
    // completes or parks on something the user has to answer.
    let advance = move |key: String, first: LoginStep| {
        leptos::task::spawn_local(async move {
            let mut step = Ok(first);
            loop {
                match step {
                    Ok(LoginStep::Complete) => {
                        session_key.set(None);
                        prompt.set(None);
                        tfa_code.set(String::new());
                        password.set(String::new());
                        on_success.run(());
                        break;
                    }
                    Ok(LoginStep::TwoFactor { prompt: p }) => {
                        // A fresh prompt means the previous code was rejected or a
                        // new one was sent; clear the stale input either way.
                        tfa_code.set(String::new());
                        session_key.set(Some(key.clone()));
                        prompt.set(Some(p));
                        break;
                    }
                    Ok(LoginStep::Pending) => {
                        session_key.set(Some(key.clone()));
                        prompt.set(None);
                        step = poll_login(key.clone()).await;
                    }
                    Err(e) => {
                        error_msg.set(Some(e.to_string()));
                        session_key.set(None);
                        prompt.set(None);
                        break;
                    }
                }
            }
            pending.set(false);
        });
    };

    let on_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let id = apple_id.get();
        let pw = password.get();
        pending.set(true);
        error_msg.set(None);

        leptos::task::spawn_local(async move {
            match begin_login(id, pw).await {
                Ok((key, step)) => advance(key, step),
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                    pending.set(false);
                }
            }
        });
    };

    // Answer the outstanding prompt with `action`.
    let act = move |action: TwoFactorAction| {
        let key = match session_key.get() {
            Some(k) => k,
            None => return,
        };
        pending.set(true);
        error_msg.set(None);
        // Hide the form while the request is in flight so the options can't be
        // double-submitted; `advance` restores it if Apple asks again.
        prompt.set(None);

        leptos::task::spawn_local(async move {
            match submit_two_factor(key.clone(), action).await {
                Ok(step) => advance(key, step),
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                    pending.set(false);
                }
            }
        });
    };

    let on_2fa_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        act(TwoFactorAction::SubmitCode(tfa_code.get()));
    };

    // Abort ends the login outright, so it resets the form rather than polling
    // for a next step the way `act` does.
    let cancel = move |_| {
        let key = match session_key.get() {
            Some(k) => k,
            None => return,
        };
        pending.set(true);
        error_msg.set(None);
        prompt.set(None);

        leptos::task::spawn_local(async move {
            // Best effort: unblocks the login thread so it tears down promptly
            // instead of sitting out its five-minute timeout.
            let _ = submit_two_factor(key, TwoFactorAction::Abort).await;
            session_key.set(None);
            tfa_code.set(String::new());
            password.set(String::new());
            pending.set(false);
        });
    };

    view! {
        {move || {
            let key = session_key.get();
            if key.is_none() {
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
                match prompt.get() {
                    // Waiting on Apple between steps.
                    None => {
                        view! {
                            <div class="tfa-step">
                                <p class="hint">"Contacting Apple..."</p>
                            </div>
                        }
                            .into_any()
                    }
                    // The previous method failed and Apple won't say which one to
                    // use, so only offer a choice of method -- a code can't be
                    // submitted from this state.
                    Some(p) if p.unknown => {
                        view! {
                            <div class="tfa-step">
                                <p>
                                    "That verification method didn't work. Choose another way to receive your code."
                                </p>
                                <TwoFactorMethods
                                    numbers=p.numbers.clone()
                                    show_devices=true
                                    pending=pending
                                    act=act
                                />
                                <button
                                    type="button"
                                    class="btn btn-secondary btn-sm"
                                    prop:disabled=pending
                                    on:click=cancel
                                >
                                    "Cancel"
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                    Some(p) => {
                        let sms = p.sms;
                        let target = p
                            .selected_number_id
                            .and_then(|id| p.numbers.iter().find(|n| n.id == id))
                            .map(|n| n.number.clone());
                        let numbers = p.numbers.clone();

                        view! {
                            <div class="tfa-step">
                                <p>
                                    {if sms {
                                        match target {
                                            Some(n) => format!("Enter the code texted to {n}."),
                                            None => "Enter the code sent by text message.".to_string(),
                                        }
                                    } else {
                                        "A two-factor code was sent to your trusted devices."
                                            .to_string()
                                    }}
                                </p>
                                <form on:submit=on_2fa_submit>
                                    <div class="form-row">
                                        <label class="form-field">
                                            "2FA Code"
                                            <input
                                                type="text"
                                                required
                                                inputmode="numeric"
                                                autocomplete="one-time-code"
                                                placeholder="000000"
                                                maxlength="6"
                                                prop:value=tfa_code
                                                on:input=move |e| tfa_code.set(event_target_value(&e))
                                            />
                                        </label>
                                    </div>
                                    <button
                                        type="submit"
                                        class="btn btn-primary"
                                        prop:disabled=pending
                                    >
                                        {move || {
                                            if pending.get() { "Verifying..." } else { "Submit Code" }
                                        }}
                                    </button>
                                </form>

                                <p class="hint tfa-alt-label">"Didn't get it?"</p>
                                <div class="tfa-methods">
                                    <button
                                        type="button"
                                        class="btn btn-secondary btn-sm"
                                        prop:disabled=pending
                                        on:click=move |_| act(TwoFactorAction::ResendCode)
                                    >
                                        "Resend code"
                                    </button>
                                </div>
                                <TwoFactorMethods
                                    numbers=numbers
                                    show_devices=sms
                                    pending=pending
                                    act=act
                                />
                                <button
                                    type="button"
                                    class="btn btn-secondary btn-sm"
                                    prop:disabled=pending
                                    on:click=cancel
                                >
                                    "Cancel"
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                }
            }
        }}

        {move || error_msg.get().map(|e| view! { <p class="error">{e}</p> })}
    }
}

/// The alternate ways to get a code: text any trusted number, or push to the
/// account's trusted devices.
#[component]
fn TwoFactorMethods<F>(
    numbers: Vec<TrustedNumberInfo>,
    /// Offer the trusted-devices route (pointless when it's already in use).
    show_devices: bool,
    pending: RwSignal<bool>,
    act: F,
) -> impl IntoView
where
    F: Fn(TwoFactorAction) + Copy + Send + Sync + 'static,
{
    if numbers.is_empty() && !show_devices {
        return ().into_any();
    }

    view! {
        <div class="tfa-methods">
            {show_devices
                .then(|| {
                    view! {
                        <button
                            type="button"
                            class="btn btn-secondary btn-sm"
                            prop:disabled=pending
                            on:click=move |_| act(TwoFactorAction::SendToDevices)
                        >
                            "Send to trusted devices"
                        </button>
                    }
                })}
            {numbers
                .into_iter()
                .map(|n| {
                    let id = n.id;
                    view! {
                        <button
                            type="button"
                            class="btn btn-secondary btn-sm"
                            prop:disabled=pending
                            on:click=move |_| act(TwoFactorAction::SendSms(id))
                        >
                            {format!("Text {}", n.number)}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
        .into_any()
}
