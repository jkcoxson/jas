use leptos::prelude::*;

use crate::app::{get_server_config, set_anisette_url};

// Baked in by build.rs from GitHub Actions environment variables.
// All are None when built locally.
const COMMIT_SHA: Option<&str> = option_env!("GIT_COMMIT_SHA");
const CI_RUN_NUMBER: Option<&str> = option_env!("CI_RUN_NUMBER");
const GIT_REF: Option<&str> = option_env!("GIT_REF_NAME");

#[component]
pub fn Settings() -> impl IntoView {
    let config = Resource::new(|| (), |_| get_server_config());

    let anisette_input = RwSignal::new(String::new());
    let anisette_saving = RwSignal::new(false);
    let anisette_msg = RwSignal::<Option<Result<(), String>>>::new(None);

    Effect::new(move |_| {
        if let Some(Ok(ref cfg)) = config.get() {
            anisette_input.set(cfg.anisette_url.clone());
        }
    });

    let on_anisette_save = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let url = anisette_input.get();
        anisette_saving.set(true);
        anisette_msg.set(None);
        leptos::task::spawn_local(async move {
            let result = set_anisette_url(url).await.map_err(|e| e.to_string());
            anisette_msg.set(Some(result));
            anisette_saving.set(false);
        });
    };

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Settings"</h1>
            </div>

            {COMMIT_SHA.map(|sha| {
                view! {
                    <div class="card">
                        <h2>"Build"</h2>
                        <table class="info-table">
                            <tbody>
                                <tr>
                                    <th>"Commit"</th>
                                    <td class="mono">{sha}</td>
                                </tr>
                                {GIT_REF.map(|r| view! {
                                    <tr>
                                        <th>"Ref"</th>
                                        <td class="mono">{r}</td>
                                    </tr>
                                })}
                                {CI_RUN_NUMBER.map(|n| view! {
                                    <tr>
                                        <th>"CI Run"</th>
                                        <td class="mono">{"#"}{n}</td>
                                    </tr>
                                })}
                            </tbody>
                        </table>
                    </div>
                }
            })}

            <Suspense fallback=|| view! { <p class="loading">"Loading..."</p> }>
                {move || {
                    config
                        .get()
                        .map(|r| match r {
                            Err(e) => {
                                view! {
                                    <p class="error">"Error loading config: " {e.to_string()}</p>
                                }
                                    .into_any()
                            }
                            Ok(cfg) => {
                                view! {
                                    <div class="card">
                                        <h2>"Server"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Bind Address"</th>
                                                    <td class="mono">{cfg.bind}</td>
                                                </tr>
                                                <tr>
                                                    <th>"Log Level"</th>
                                                    <td>{cfg.log_level}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"Storage"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Database"</th>
                                                    <td class="mono">{cfg.database_path}</td>
                                                </tr>
                                                <tr>
                                                    <th>"IPA Directory"</th>
                                                    <td class="mono">{cfg.ipa_dir}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"Refresh Scheduler"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Check Interval"</th>
                                                    <td>
                                                        {format!("Every {} hours", cfg.interval_hours)}
                                                    </td>
                                                </tr>
                                                <tr>
                                                    <th>"Refresh Window"</th>
                                                    <td>
                                                        {format!(
                                                            "{} days before expiry",
                                                            cfg.refresh_window_days,
                                                        )}
                                                    </td>
                                                </tr>
                                                <tr>
                                                    <th>"Worker Threads"</th>
                                                    <td>{cfg.worker_threads}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"mDNS Discovery"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Enabled"</th>
                                                    <td>
                                                        {if cfg.mdns_enabled { "Yes" } else { "No" }}
                                                    </td>
                                                </tr>
                                                {(!cfg.mdns_interface.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <tr>
                                                                <th>"Interface"</th>
                                                                <td class="mono">{cfg.mdns_interface}</td>
                                                            </tr>
                                                        }
                                                    })}
                                            </tbody>
                                        </table>
                                    </div>
                                    <p class="hint">
                                        "To change settings, edit " <code>"jas.toml"</code>
                                        " (or set " <code>"JAS_CONFIG"</code>
                                        " to a custom path) and restart JAS."
                                    </p>
                                    <div class="card">
                                        <h2>"Anisette Server"</h2>
                                        <form on:submit=on_anisette_save>
                                            <div class="form-row">
                                                <label class="form-field">
                                                    "Server URL"
                                                    <input
                                                        type="url"
                                                        required
                                                        placeholder="https://ani.stikstore.app"
                                                        prop:value=anisette_input
                                                        on:input=move |e| {
                                                            anisette_input.set(event_target_value(&e))
                                                        }
                                                    />
                                                </label>
                                            </div>
                                            <button
                                                type="submit"
                                                class="btn btn-primary"
                                                prop:disabled=anisette_saving
                                            >
                                                {move || {
                                                    if anisette_saving.get() { "Saving..." } else { "Save" }
                                                }}
                                            </button>
                                        </form>
                                        {move || {
                                            anisette_msg.get().map(|r| match r {
                                                Ok(()) => view! {
                                                    <p class="success form-msg">"Saved."</p>
                                                }
                                                    .into_any(),
                                                Err(e) => view! {
                                                    <p class="error form-msg">{e}</p>
                                                }
                                                    .into_any(),
                                            })
                                        }}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}
