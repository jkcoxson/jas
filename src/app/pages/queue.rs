use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{list_jobs, QueueEntry};

fn format_ts(ts: Option<i64>) -> String {
    match ts {
        Some(ts) => chrono::DateTime::from_timestamp(ts, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        None => "-".to_string(),
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "done" => "job-badge job-done",
        "failed" => "job-badge job-failed",
        "running" => "job-badge job-running",
        _ => "job-badge job-queued",
    }
}

#[component]
pub fn Queue() -> impl IntoView {
    let trigger = RwSignal::new(0u32);
    let jobs = Resource::new(move || trigger.get(), |_| list_jobs());

    let has_active = move || {
        jobs.get()
            .and_then(|r| r.ok())
            .map(|js| js.iter().any(|j| j.status == "queued" || j.status == "running"))
            .unwrap_or(false)
    };

    // Re-poll every 3s while any job is queued or running.
    Effect::new(move |_| {
        if has_active() {
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
                        3_000,
                    );
                cb.forget();
            }
        }
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Job Queue"</h1>
                <button
                    class="btn btn-sm btn-secondary"
                    on:click=move |_| trigger.update(|n| *n += 1)
                >
                    "Refresh"
                </button>
            </div>

            <Suspense fallback=|| view! { <p class="loading">"Loading jobs…"</p> }>
                {move || {
                    jobs.get()
                        .map(|result| match result {
                            Err(e) => {
                                view! {
                                    <p class="error">"Failed to load queue: " {e.to_string()}</p>
                                }
                                    .into_any()
                            }
                            Ok(entries) if entries.is_empty() => {
                                view! { <p class="muted">"No jobs yet."</p> }.into_any()
                            }
                            Ok(entries) => {
                                view! {
                                    <table class="table">
                                        <thead>
                                            <tr>
                                                <th>"App"</th>
                                                <th>"Device"</th>
                                                <th>"Kind"</th>
                                                <th>"Status"</th>
                                                <th>"Started"</th>
                                                <th>"Finished"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {entries
                                                .into_iter()
                                                .map(|j: QueueEntry| {
                                                    let device_href = format!(
                                                        "/devices/{}",
                                                        j.device_id,
                                                    );
                                                    let is_running = j.status == "running";
                                                    let badge_class = status_badge_class(
                                                        &j.status,
                                                    );
                                                    let progress = j.progress;
                                                    let stage = j.stage.clone();
                                                    let error = j.error.clone();
                                                    view! {
                                                        <tr>
                                                            <td>{j.app_name}</td>
                                                            <td>
                                                                <A
                                                                    href=device_href
                                                                    attr:class="nav-link"
                                                                >
                                                                    {j.device_name}
                                                                </A>
                                                            </td>
                                                            <td class="mono">{j.kind}</td>
                                                            <td>
                                                                <span class=badge_class>
                                                                    {j.status.clone()}
                                                                </span>
                                                                {is_running
                                                                    .then(|| {
                                                                        stage
                                                                            .map(|s| {
                                                                                view! {
                                                                                    <span class="job-stage">
                                                                                        " - " {s}
                                                                                    </span>
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
                                                                {error
                                                                    .map(|e| {
                                                                        view! {
                                                                            <div class="error">{e}</div>
                                                                        }
                                                                    })}
                                                            </td>
                                                            <td>{format_ts(j.started_at)}</td>
                                                            <td>{format_ts(j.finished_at)}</td>
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
        </div>
    }
}
