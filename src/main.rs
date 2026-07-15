#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tokio::sync::mpsc;
    use tracing::info;

    use jas::{
        app::App,
        server::{
            config::Config,
            db::create_pool,
            mdns::run_mdns,
            scheduler::{run_scheduler, run_workers},
            state::AppState,
        },
    };

    // Load config.
    let config = Config::load();

    // Init logging.
    let log_level = config.server.log_level.clone();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level)),
        )
        .init();

    info!("JAS starting");

    // Create DB pool and run migrations.
    let pool = create_pool(&config).await.expect("Database init failed");
    info!("Database ready at {}", config.storage.database_path);

    let anisette_url = sqlx::query_scalar!(
        "SELECT value FROM sideload_storage WHERE key = '_server/anisette_url'"
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "https://ani.stikstore.app".to_string());
    info!("Anisette server: {anisette_url}");

    // Derive encryption key.
    let key = config.secret_key_bytes();

    // Job queue channel.
    let (job_tx, job_rx) = mpsc::channel::<jas::server::state::JobRequest>(256);

    let app_state = AppState::new(pool, config.clone(), key, job_tx, anisette_url);

    // Spawn the refresh scheduler.
    {
        let state = app_state.clone();
        tokio::spawn(run_scheduler(state));
    }

    // Spawn the job worker pool.
    {
        let state = app_state.clone();
        tokio::spawn(run_workers(state, job_rx));
    }

    // Spawn the mDNS browser. It self-exits if discovery.mdns_enabled = false.
    {
        let state = app_state.clone();
        tokio::spawn(run_mdns(state));
    }

    // Build Leptos + Axum router.
    let conf = get_configuration(None).unwrap();
    let addr = match config.server.bind.parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(
                "Invalid server.bind {:?} in jas.toml ({e}); falling back to Leptos site_addr",
                config.server.bind
            );
            conf.leptos_options.site_addr
        }
    };
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    {
        let state = app_state.clone();
        let leptos_options = leptos_options.clone();
        let app = Router::new()
            .leptos_routes_with_context(
                &leptos_options,
                routes,
                move || {
                    provide_context(state.clone());
                },
                {
                    let leptos_options = leptos_options.clone();
                    move || jas::app::shell(leptos_options.clone())
                },
            )
            .fallback(leptos_axum::file_and_error_handler(jas::app::shell))
            .with_state(leptos_options);

        info!("Listening on http://{addr}");
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app.into_make_service()).await.unwrap();
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
