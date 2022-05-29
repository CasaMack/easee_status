use std::{sync::Arc, fmt::Display};

use chrono::{DateTime, Local};
use tokio::sync::Mutex;
use tracing::instrument;
use super::logic::{get_charger_state, EaseeError, SessionState, ChargerState};
use rocket::{http::Status, serde::{json::Json}, response::{Redirect, status}, get, request::FromParam};
use rocket::State;

#[derive(Debug)]
pub struct Cache {
    last_update: Mutex<Option<DateTime<Local>>>,
    state: Mutex<Option<Vec<ChargerState>>>,
}

impl Cache {
    fn new() -> Self {
        Cache {
            last_update: Mutex::new(None),
            state: Mutex::new(None),
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum Field {
    Power,
    Session,
    Energy,
}

impl Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Field::Power => write!(f, "power"),
            Field::Session => write!(f, "session"),
            Field::Energy => write!(f, "energy"),
        }
    }
}

impl<'a> FromParam<'a> for Field {
    type Error = &'static str;

    fn from_param(param: &'a str) -> Result<Self, Self::Error> {
        match param {
            "power" => Ok(Field::Power),
            "session" => Ok(Field::Session),
            "energy" => Ok(Field::Energy),
            _ => Err("Invalid field"),
        }
    }
}

#[instrument(skip(session_state, cache))]
#[get("/")]
pub async fn index(session_state: &State<Arc<Mutex<SessionState>>>, cache: &State<Cache>) -> status::Custom<Json<Vec<ChargerState>>> {
    tracing::info!("Handling request");
    let charger_states = get_charger_state(session_state.inner().to_owned()).await;
    match charger_states {
        Ok(chargers) => {
            tracing::debug!("Got charger states: {}", chargers.len());
            for charger in &chargers {
                tracing::trace!("{:?}", charger);
            }
            tracing::info!("Ok response");
            let mut mtx = cache.state.lock().await;
            *mtx = Some(chargers.clone());
            let mut mtx = cache.last_update.lock().await;
            *mtx = Some(Local::now());
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

#[instrument]
#[get("/carChargerUsage")]
pub async fn car_charger_usage() -> Redirect {
    tracing::debug!("Redirecting carChargerUsage -> power");
    Redirect::to("/power")
}

#[instrument]
#[get("/easeeLadeMengde")]
pub async fn easee_lade_mengde() -> Redirect {
    tracing::debug!("Redirecting easeeLadeMengde -> session");
    Redirect::to("/session")
}

#[instrument]
#[get("/easeeEnergyPerHour")]
pub async fn easee_energy_per_hour() -> Redirect {
    tracing::debug!("Redirecting easeeEnergyPerHour -> energy");
    Redirect::to("/energy")
}

#[instrument]
#[get("/<field>")]
pub async fn field(field: Field) -> Redirect {
    tracing::debug!("Redirecting {} -> /{}/0", field, field);
    Redirect::to(format!("/{}/0", field))
}

#[instrument(skip(session_state, cache))]
#[get("/<field>/<index>")]
pub async fn field_index(session_state: &State<Arc<Mutex<SessionState>>>, cache: &State<Cache>, field: Field, index: usize) -> status::Custom<String> {
    tracing::debug!("Serving {}/{}", field, index);
    let last_update = cache.last_update.lock().await;
    if let Some(last_update) = last_update.as_ref() {
        let now = Local::now();
        let next_refresh_time = last_update.checked_add_signed(chrono::Duration::minutes(1));
        if let Some(next_refresh_time) = next_refresh_time {
            if now > next_refresh_time {
                let chargers = get_charger_state(session_state.inner().to_owned()).await;
                match chargers {
                    Ok(chargers) => {
                        let mut mtx = cache.state.lock().await;
                        *mtx = Some(chargers.clone());
                        let mut mtx = cache.last_update.lock().await;
                        *mtx = Some(Local::now());
                        if let Some(charger) = chargers.get(index) {
                            tracing::info!("Ok response");
                            status::Custom(Status::Ok, format!("{}", match field {
                                Field::Power => charger.power,
                                Field::Session => charger.session,
                                Field::Energy => charger.energy_per_hour,
                            }))
                        } else {
                            tracing::info!("Requested index out of range");
                            status::Custom(Status::BadRequest, "Index out of range".to_string())
                        }
                    }
                    Err(e) => {
                        tracing::info!("Error response");
                        match e {
                            EaseeError::Unathorized => status::Custom(Status::Unauthorized, "".to_string()),
                            EaseeError::LoginFailed => status::Custom(Status::InternalServerError, "".to_string()),
                            EaseeError::HttpFailed => status::Custom(Status::InternalServerError, "".to_string()),
                            EaseeError::InvalidResponse => status::Custom(Status::InternalServerError, "".to_string()),
                            EaseeError::RateLimit => status::Custom(Status::TooManyRequests, "".to_string()),
                        }
                    }
                }
            } else {
                tracing::info!("Using cached values");
                let chargers = cache.state.lock().await;
                if let Some(chargers) = chargers.as_ref() {
                    if let Some(charger) = chargers.get(index) {
                        tracing::info!("Ok response");
                        status::Custom(Status::Ok, format!("{}", match field {
                            Field::Session => charger.session,
                            Field::Power => charger.power,
                            Field::Energy => charger.energy_per_hour,
                        }))
                    } else {
                        tracing::info!("Requested index out of range");
                        status::Custom(Status::BadRequest, "Index out of range".to_string())
                    }
                } else {
                    tracing::error!("No charger state in cache, but Some(last_update)");
                    status::Custom(Status::BadRequest, "No cached data".to_string())
                }
            }
        } else {
            tracing::error!("Chrono overflowed");
            status::Custom(Status::InternalServerError, "".to_string())
        }
    
    } else {
        tracing::info!("First request");
        let chargers = get_charger_state(session_state.inner().to_owned()).await;
        match chargers {
            Ok(chargers) => {
                let mut mtx = cache.state.lock().await;
                *mtx = Some(chargers.clone());
                let mut mtx = cache.last_update.lock().await;
                *mtx = Some(Local::now());
                if let Some(charger) = chargers.get(index) {
                    tracing::info!("Ok response");
                    status::Custom(Status::Ok, format!("{}", match field {
                        Field::Session => charger.session,
                        Field::Power => charger.power,
                        Field::Energy => charger.energy_per_hour,
                    }))
                } else {
                    tracing::info!("Requested index out of range");
                    status::Custom(Status::BadRequest, "Index out of range".to_string())
                }
            }
            Err(e) => {
                tracing::info!("Error response");
                match e {
                    EaseeError::Unathorized => status::Custom(Status::Unauthorized, "".to_string()),
                    EaseeError::LoginFailed => status::Custom(Status::InternalServerError, "".to_string()),
                    EaseeError::HttpFailed => status::Custom(Status::InternalServerError, "".to_string()),
                    EaseeError::InvalidResponse => status::Custom(Status::InternalServerError, "".to_string()),
                    EaseeError::RateLimit => status::Custom(Status::TooManyRequests, "".to_string()),
                }
            }
        }
    }
}