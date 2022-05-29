use std::sync::Arc;

use easee_status::v1::{get_charger_state, EaseeError, SessionState, ChargerState};
use rocket::{http::Status, serde::json::Json};
use rocket::State;
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::{FmtSubscriber, fmt::format::FmtSpan};
use rocket::response::status;

#[macro_use]
extern crate rocket;

#[get("/")]
async fn index(session_state: &State<Arc<Mutex<SessionState>>>) -> status::Custom<Json<Vec<ChargerState>>> {
    tracing::info!("Handling request");
    let charger_states = get_charger_state(session_state.inner().to_owned()).await;
    match charger_states {
        Ok(chargers) => {
            tracing::debug!("Got charger states: {}", chargers.len());
            for charger in &chargers {
                tracing::trace!("{:?}", charger);
            }
            tracing::info!("Ok response");
            status::Custom(Status::Ok, Json(chargers))
        }
        Err(e) => {
            tracing::info!("Error response");
            match e {
                EaseeError::Unathorized => status::Custom(Status::Unauthorized, Json(Vec::new())),
                EaseeError::LoginFailed => status::Custom(Status::InternalServerError, Json(Vec::new())),
                EaseeError::HttpFailed => status::Custom(Status::InternalServerError, Json(Vec::new())),
                EaseeError::InvalidResponse => status::Custom(Status::InternalServerError, Json(Vec::new())),
                EaseeError::RateLimit => status::Custom(Status::TooManyRequests, Json(Vec::new())),
            }
        }
    }
}


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
        .mount("/", routes![index])
        .launch()
        .await?;
    tracing::info!("Extinguished rocket");
    Ok(())
}