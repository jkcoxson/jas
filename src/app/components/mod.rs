// Shared UI components.

/// Formats a remaining duration (in seconds) as a single rounded-up unit:
/// days while >= 1 day, then hours while >= 1 hour, then minutes while >= 1
/// minute, then seconds. Always rounds up (e.g. 6.1 days -> "7d", 5.9 days ->
/// "6d") so the counter never understates how much time is left.
pub fn round_up_duration(remaining_secs: i64) -> String {
    if remaining_secs <= 0 {
        return "Expired".to_string();
    }
    if remaining_secs >= 86400 {
        let days = (remaining_secs as f64 / 86400.0).ceil() as i64;
        format!("{days}d")
    } else if remaining_secs >= 3600 {
        let hours = (remaining_secs as f64 / 3600.0).ceil() as i64;
        format!("{hours}h")
    } else if remaining_secs >= 60 {
        let mins = (remaining_secs as f64 / 60.0).ceil() as i64;
        format!("{mins}m")
    } else {
        format!("{remaining_secs}s")
    }
}

/// Prompt for destrictive actions, calls the browser's alert and
/// returns what the user chooses.
pub fn confirm(message: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.confirm_with_message(message).ok())
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = message;
        false
    }
}
