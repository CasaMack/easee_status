use std::{env, sync::Arc};

use tokio::{self, sync::Mutex};
use tracing::Level;

use easee_status::{get_db_info, tick};
use easee_status::{v1::run::get_logger, SessionState};

#[tokio::main]
async fn main() {
    let (subscriber, _guard) = get_logger();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    tracing::trace!("Log setup complete");
    let (db_addr, db_name) = get_db_info();

    let s = tracing::span!(Level::TRACE, "main");
    let _guard = s.enter();

    let mut interval_timer = tokio::time::interval(
        chrono::Duration::minutes(
            env::var("INTERVAL").map_or(1, |i| i.parse().expect("Illegal interval format")),
        )
        .to_std()
        .unwrap(),
    );
    let login_state = Arc::new(Mutex::new(SessionState::new()));
    loop {
        interval_timer.tick().await;

        tokio::spawn(tick(login_state.clone(), db_addr.clone(), db_name.clone()));
    }
}
