use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::{FmtSubscriber, fmt::format::FmtSpan};

use easee_status::v1::{routes::*, logic::SessionState};

#[macro_use]
extern crate rocket;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {

    let appender = tracing_appender::rolling::daily("./var/log", "easee-status-server");
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(appender);
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_span_events(FmtSpan::ACTIVE)
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_writer(non_blocking_appender)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let s = tracing::span!(Level::TRACE, "main");
    let _guard = s.enter();

    let state = SessionState::new();
    let state = Arc::new(Mutex::new(state));
    tracing::info!("Igniting rocket");
    let _rocket = rocket::build()
        .manage(state.clone())
        .manage(Cache::default())
        .mount("/", routes![
            index,
            field,
            field_index,
            car_charger_usage,
            easee_lade_mengde,
            easee_energy_per_hour, 
            
        ])
        .launch()
        .await?;
    tracing::info!("Extinguished rocket");
    Ok(())
}