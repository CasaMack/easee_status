use std::{env, sync::Arc};

use chrono::Utc;
use influxdb::{Client, InfluxDbWriteable};
use tokio::sync::Mutex;
use tracing::{instrument, metadata::LevelFilter, Level};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{
    fmt::format::{DefaultFields, FmtSpan, Format},
    FmtSubscriber,
};

use crate::v1::{easee::get_charger_state, structs::Variable};

use super::structs::SessionState;

#[instrument]
pub fn get_db_info() -> (Arc<String>, Arc<String>) {
    let db_addr = env::var("INFLUXDB_ADDR").expect("INFLUXDB_ADDR not set");
    tracing::info!("INFLUXDB_ADDR: {}", db_addr);

    let db_name = env::var("INFLUXDB_DB_NAME").expect("INFLUXDB_DB_NAME not set");
    tracing::info!("INFLUXDB_DB_NAME: {}", db_name);

    (Arc::new(db_addr), Arc::new(db_name))
}

pub fn get_logger() -> (
    FmtSubscriber<DefaultFields, Format, LevelFilter, NonBlocking>,
    WorkerGuard,
) {
    let appender = tracing_appender::rolling::daily("./var/log", "easee-status-server");
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(appender);

    let level = match env::var("LOG_LEVEL") {
        Ok(l) => match l.as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::INFO,
        },
        Err(_) => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_span_events(FmtSpan::NONE)
        .with_ansi(false)
        .with_max_level(level)
        .with_writer(non_blocking_appender)
        // completes the builder.
        .finish();

    (subscriber, guard)
}

#[instrument(skip_all, level = "trace")]
pub async fn tick(
    login_state: Arc<Mutex<SessionState>>,
    db_addr: Arc<String>,
    db_name: Arc<String>,
) {
    tracing::debug!("tick");
    let charger_state = get_charger_state(login_state).await;
    match charger_state {
        Ok(state) => {
            tracing::info!("Writing {} states", state.len());
            let client = Client::new(db_addr.as_str(), db_name.as_str());
            for charger in state {
                tracing::trace!("Writing power");
                write_to_db(&client, "power", charger.power, &charger.id).await;
                tracing::trace!("Writing enrgy_per_hour");
                write_to_db(
                    &client,
                    "energy_per_hour",
                    charger.energy_per_hour,
                    &charger.id,
                )
                .await;
                tracing::trace!("Writing session");
                write_to_db(&client, "session", charger.session, &charger.id).await;
            }
        }
        Err(e) => {
            tracing::error!("error getting charger state: {}", e);
        }
    }
}

#[instrument(skip(client), level = "trace")]
async fn write_to_db(client: &Client, name: &str, value: f64, measurement: &str) {
    let variable = Variable {
        time: Utc::now(),
        value,
        variable: String::from(name),
    };

    let write_result = client.query(variable.into_query(measurement)).await;
    match write_result {
        Ok(_) => {
            tracing::trace!("Writing {} success", name);
        }
        Err(e) => {
            tracing::warn!("Writing {} failed: {}", name, e);
        }
    }
}
