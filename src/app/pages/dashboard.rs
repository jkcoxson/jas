use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::components::round_up_duration;
use crate::app::{dashboard_summary, DeviceSummary, RefreshDevice};

#[component]
pub fn Dashboard() -> impl IntoView {
    let summaries = Resource::new(|| (), |_| dashboard_summary());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Dashboard"</h1>
            </div>

            <Suspense fallback=|| {
                view! { <p class="loading">"Loading devices…"</p> }
            }>
                {move || {
                    summaries
                        .get()
                        .map(|result| match result {
                            Err(e) => {
                                view! {
                                    <p class="error">"Failed to load devices: " {e.to_string()}</p>
                                }
                                    .into_any()
                            }
                            Ok(devs) if devs.is_empty() => {
                                view! {
                                    <div class="empty-state">
                                        <p>"No devices registered yet."</p>
                                        <A href="/devices" attr:class="btn btn-primary">
                                            "Add a Device"
                                        </A>
                                    </div>
                                }
                                    .into_any()
                            }
                            Ok(devs) => {
                                view! {
                                    <div class="device-grid">
                                        {devs
                                            .into_iter()
                                            .map(|s| view! { <DeviceCard summary=s /> })
                                            .collect_view()}
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

#[component]
fn DeviceCard(summary: DeviceSummary) -> impl IntoView {
    let device = summary.device;
    let href = format!("/devices/{}", device.id);
    let device_id = device.id.clone();
    let last_seen = device.last_seen.map(|ts| {
        chrono::DateTime::from_timestamp(ts, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M")
            .to_string()
    });

    let now = chrono::Utc::now().timestamp();
    let app_count = summary.app_count;
    let expiring_soon = summary.expiring_soon_count;
    let expired = summary.expired_count;
    let next_line = expiry_line(summary.next_expires_at, app_count, expired, now);
    let next_class = expiry_class(
        summary.next_expires_at,
        app_count,
        expired,
        summary.expiring_soon_threshold_secs,
        now,
    );
    let counts_line = counts_line(expiring_soon, expired);

    let refresh_action = ServerAction::<RefreshDevice>::new();

    view! {
        <div class="device-card">
            <div class="device-card-header">
                <span class="device-name">{device.name.clone()}</span>
            </div>
            <div class="device-card-body">
                <p class="device-ip">{device.ip.clone()}</p>
                <p class="device-udid">"UDID: " {device.udid}</p>
                {last_seen.map(|t| view! { <p class="device-last-seen">"Last seen: " {t}</p> })}
                <div class="device-summary">
                    <p class="summary-count">
                        {app_count} " app" {if app_count == 1 { "" } else { "s" }}
                    </p>
                    <p class=next_class>{next_line}</p>
                    {counts_line.map(|s| view! { <p class="summary-counts">{s}</p> })}
                </div>
                {move || {
                    refresh_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(0) => view! { <p class="muted">"Nothing to refresh."</p> }.into_any(),
                            Ok(n) => {
                                view! {
                                    <p class="muted">
                                        {n} " refresh job" {if n == 1 { "" } else { "s" }}
                                        " queued."
                                    </p>
                                }
                                    .into_any()
                            }
                            Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                        })
                }}
            </div>
            <div class="device-card-footer">
                <A href=href attr:class="btn btn-secondary">
                    "Manage Apps"
                </A>
                {(app_count > 0)
                    .then(|| {
                        view! {
                            <ActionForm action=refresh_action>
                                <input type="hidden" name="device_id" value=device_id.clone() />
                                <button
                                    type="submit"
                                    class="btn btn-primary"
                                    prop:disabled=move || refresh_action.pending().get()
                                >
                                    {move || {
                                        if refresh_action.pending().get() {
                                            "Queuing..."
                                        } else {
                                            "Refresh All"
                                        }
                                    }}
                                </button>
                            </ActionForm>
                        }
                    })}
            </div>
        </div>
    }
}

fn expiry_line(
    next_expires_at: Option<i64>,
    app_count: usize,
    expired_count: usize,
    now: i64,
) -> String {
    if app_count == 0 {
        return "No apps tracked".to_string();
    }
    match next_expires_at {
        Some(ts) => format!("Next expires in {}", round_up_duration(ts - now)),
        None if expired_count > 0 => "All expired".to_string(),
        None => "—".to_string(),
    }
}

fn expiry_class(
    next_expires_at: Option<i64>,
    app_count: usize,
    expired_count: usize,
    threshold_secs: i64,
    now: i64,
) -> &'static str {
    if app_count == 0 {
        return "summary-next muted";
    }
    match next_expires_at {
        Some(ts) if ts - now <= threshold_secs => "summary-next warn",
        Some(_) => "summary-next",
        None if expired_count > 0 => "summary-next danger",
        None => "summary-next muted",
    }
}

fn counts_line(expiring_soon: usize, expired: usize) -> Option<String> {
    if expiring_soon == 0 && expired == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if expiring_soon > 0 {
        parts.push(format!("{expiring_soon} expiring soon"));
    }
    if expired > 0 {
        parts.push(format!("{expired} expired"));
    }
    Some(parts.join(", "))
}
