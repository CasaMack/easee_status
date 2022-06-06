use std::{collections::HashMap, env, sync::Arc};

use chrono::{prelude::*, Duration};

use tokio::sync::Mutex;
use tracing::{debug, error, instrument, span, trace, warn, Level, info};

use local_credentials;

use super::structs::{ChargerState, EaseeError, SessionState};

const EASEE_BASE: &'static str = "https://api.easee.cloud/api";
const CHARGERS_ENDPOINT: &'static str = "https://api.easee.cloud/api/chargers";
const LOGIN_ENDPOINT: &'static str = "https://api.easee.cloud/api/accounts/login";
const REFRESH_ENDPOINT: &'static str = "https://api.easee.cloud/api/accounts/refresh_token";

#[instrument(skip_all, level = "trace")]
pub async fn get_charger_state(
    session: Arc<Mutex<SessionState>>,
) -> Result<Vec<ChargerState>, EaseeError> {
    let ids = get_charger_list(session.to_owned()).await;
    if let Err(e) = ids {
        debug!("Bubbling error: {}", e);
        return Err(e);
    }
    let ids = ids.unwrap();
    let mut states = Vec::new();
    for id in ids {
        trace!("Getting charger state charger: {}", &id);
        let state = external_request_charger_state(&id, session.to_owned()).await;
        if let Err(e) = state {
            return Err(e);
        }
        trace!("Pushing charger state charger: {}", &id);
        states.push(state.unwrap());
    }
    Ok(states)
}

#[instrument(skip_all, level = "trace")]
async fn get_charger_list(session: Arc<Mutex<SessionState>>) -> Result<Vec<String>, EaseeError> {
    refresh_auth(session.to_owned()).await?;
    let client = reqwest::Client::new();
    if let Some(ref t) = session.lock().await.token {
        let res = client
            .get(CHARGERS_ENDPOINT)
            .bearer_auth(t)
            .send()
            .await
            .map_err(|_| EaseeError::HttpFailed)?;
        if res.status().is_success() {
            let mut charger_ids = Vec::new();

            let parsing_span = span!(Level::TRACE, "parsing_response");
            {
                let _guard = parsing_span.enter();

                let body = res.text().await.map_err(|_| EaseeError::HttpFailed)?;

                let json: serde_json::Value =
                    serde_json::from_str(&body).map_err(|_| EaseeError::InvalidResponse)?;
                for charger in json.as_array().unwrap() {
                    let id = charger
                        .get("id")
                        .ok_or(EaseeError::InvalidResponse)?
                        .as_str()
                        .ok_or(EaseeError::InvalidResponse)?
                        .to_string();
                    trace!("Got charger: {:?}", id);
                    charger_ids.push(id);
                }
                debug!("Got {} chargers", charger_ids.len());
            }
            Ok(charger_ids)
        } else {
            if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                warn!("Rate limit exceeded");
                Err(EaseeError::Unathorized)
            } else {
                error!("Request failed: {}", res.status());
                Err(EaseeError::HttpFailed)
            }
        }
    } else {
        error!("No token after refresh");
        unreachable!();
    }
}

#[instrument(skip(session), level = "trace")]
async fn external_request_charger_state(
    charger_id: &str,
    session: Arc<Mutex<SessionState>>,
) -> Result<ChargerState, EaseeError> {
    refresh_auth(session.to_owned()).await?;

    let url = format!("{}/chargers/{}/state", EASEE_BASE, charger_id);
    let client = reqwest::Client::new();
    if let Some(ref t) = session.lock().await.token {
        trace!("Using token: {}", t);
        let res = client
            .get(&url)
            .bearer_auth(t)
            .send()
            .await
            .map_err(|_| EaseeError::HttpFailed)?;
        if res.status().is_success() {
            trace!("Request success");
            let charger_state;

            let parsing_span = span!(Level::TRACE, "parsing_response");
            {
                let _guard = parsing_span.enter();

                let body = res.text().await.map_err(|_| EaseeError::HttpFailed)?;

                let json: serde_json::Value =
                    serde_json::from_str(&body).map_err(|_| EaseeError::InvalidResponse)?;
                let power = json["totalPower"]
                    .as_f64()
                    .ok_or(EaseeError::InvalidResponse)?;
                let session = json["sessionEnergy"]
                    .as_f64()
                    .ok_or(EaseeError::InvalidResponse)?;
                let energy_per_hour = json["energyPerHour"]
                    .as_f64()
                    .ok_or(EaseeError::InvalidResponse)?;
                charger_state = ChargerState {
                    id: charger_id.to_string(),
                    power,
                    session,
                    energy_per_hour,
                };
                debug!("Got charger state: {:?}", charger_state);
            }
            return Ok(charger_state);
        } else {
            if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                warn!("Rate limit exceeded");
                Err(EaseeError::RateLimit)
            } else {
                error!("Request failed: {}", res.status());
                Err(EaseeError::Unathorized)
            }
        }
    } else {
        error!("No token after refresh");
        unreachable!();
    }
}

#[instrument(skip_all, ret, level = "trace")]
async fn login(session: Arc<Mutex<SessionState>>) -> Result<(), EaseeError> {
    tracing::trace!("Creating client");
    let client = reqwest::Client::builder();
    tracing::trace!("building client");
    let client = client.build();
    tracing::trace!("client built");
    if let Err(e) = &client {
        tracing::error!("Failed to create client: {}", e);
    }
    let client = client.unwrap();

    let mut payload = HashMap::new();

    tracing::trace!("Attempt to get credentials from env");
    let usr = env::var("USERNAME");
    let pwd = env::var("PASSWORD");
    if usr.is_ok() && pwd.is_ok() {
        tracing::info!("Credentials loaded from env");
        payload.insert("username", usr.unwrap());
        payload.insert("password", pwd.unwrap());
        tracing::trace!("Inserted credentials");
    } else {
        tracing::trace!("Credentials not found in env");
        tracing::trace!("Attempt to load credentials");
        let file = env::var("CREDENTIALS_FILE").ok();
        let file_str = (&file).as_ref().map(|x| x.as_str());
        let creds = local_credentials::async_get_credentials(file_str)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load credentials: {}", e);
                EaseeError::LoginFailed
            })?;
        tracing::info!("Credentials loaded from file");
        payload.insert("userName", creds.username);
        payload.insert("password", creds.password);
        tracing::trace!("Inserted credentials");
    }

    debug!("Sending login request");
    let response = client
        .post(LOGIN_ENDPOINT)
        .json(&payload)
        .header("Content-type", "application/json")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to send login request: {}", e);
            EaseeError::HttpFailed
        })?;

    if response.status().is_success() {
        let body = response.text().await.map_err(|_| EaseeError::HttpFailed)?;
        debug!("Got response: {}", body);

        let parsing_span = span!(Level::TRACE, "parsing_response");
        {
            let _guard = parsing_span.enter();

            let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                tracing::error!("Failed to parse response: {}", e);
                EaseeError::InvalidResponse
            })?;

            let token = json["accessToken"].as_str().ok_or_else(|| {
                tracing::error!("Error accessing field accessToken");
                EaseeError::InvalidResponse
            })?;
            let refresh_token = json["refreshToken"].as_str().ok_or_else(|| {
                tracing::error!("Error accessing field refreshToken");
                EaseeError::InvalidResponse
            })?;
            let duration = json["expiresIn"].as_i64().ok_or_else(|| {
                tracing::error!("Error accessing field expiresIn");
                EaseeError::InvalidResponse
            })?;

            let mut mutex_guard = session.lock().await;
            mutex_guard.token = Some(token.to_string());
            mutex_guard.refresh_token = Some(refresh_token.to_string());
            mutex_guard.lifetime = Some(
                Local::now()
                    .checked_add_signed(Duration::seconds(duration))
                    .ok_or_else(|| {
                        tracing::error!("Chrono overflow");
                        EaseeError::InvalidResponse
                    })?,
            );
            debug!("Token: {}", token);
        }

        info!("Login success");
        Ok(())
    } else {
        error!(
            "Login failed: {:?}: {:?}",
            response.status(),
            response.status().canonical_reason()
        );
        Err(EaseeError::LoginFailed)
    }
}

#[instrument(skip_all, level = "trace")]
async fn refresh_token(session: Arc<Mutex<SessionState>>) -> Result<(), EaseeError> {
    let client = reqwest::Client::new();

    let mut payload = HashMap::new();

    if session.lock().await.refresh_token.is_none() {
        warn!("No refresh token, logging in");
        login(session).await?;
        return Ok(());
    }

    if session.lock().await.token.is_none() {
        warn!("No token, logging in");
        login(session).await?;
        return Ok(());
    }

    let response;
    // Ok to unwrap as the two checks above *should* ensure they are `Some`. If they fail the function will return before this point.
    {
        let mutex_guard = session.lock().await;
        let refresh_token = mutex_guard.refresh_token.as_ref().unwrap();
        let token = mutex_guard.token.as_ref().unwrap();
        payload.insert("refreshToken", refresh_token);
        payload.insert("accessToken", token);

        debug!("Sending token refresh request");
        response = client
            .post(REFRESH_ENDPOINT)
            .json(&payload)
            .header("Content-type", "application/json")
            .send()
            .await
            .map_err(|_| EaseeError::HttpFailed)?;
    }
    if response.status().is_success() {
        let body = response.text().await.map_err(|_| EaseeError::HttpFailed)?;
        debug!("Got response: {}", body);

        let parsing_span = span!(Level::TRACE, "parsing_response");
        {
            let _guard = parsing_span.enter();

            let json: serde_json::Value =
                serde_json::from_str(&body).map_err(|_| EaseeError::InvalidResponse)?;

            let token = json["accessToken"]
                .as_str()
                .ok_or(EaseeError::InvalidResponse)?;
            let refresh_token = json["refreshToken"]
                .as_str()
                .ok_or(EaseeError::InvalidResponse)?;
            let duration = json["expiresIn"]
                .as_i64()
                .ok_or(EaseeError::InvalidResponse)?;
            let mut mutex_guard = session.lock().await;
            mutex_guard.token = Some(token.to_string());
            mutex_guard.refresh_token = Some(refresh_token.to_string());
            mutex_guard.lifetime = Some(
                Local::now()
                    .checked_add_signed(Duration::seconds(duration))
                    .ok_or(EaseeError::InvalidResponse)?,
            );
            debug!("Token: {}", token);
        }

        info!("Token refreshed");
        Ok(())
    } else {
        error!("Token refresh failed");
        Err(EaseeError::LoginFailed)
    }
}

#[instrument(skip_all, level = "trace")]
async fn refresh_auth(session: Arc<Mutex<SessionState>>) -> Result<(), EaseeError> {
    let mutex_guard = session.lock().await;
    if mutex_guard.token.is_some() && mutex_guard.lifetime.is_some() {
        // Safe to unwrap as above checks that lifetime is some
        if mutex_guard.lifetime.unwrap() > Local::now() {
            debug!("Token is still valid");
        } else {
            debug!("Token expired");
            drop(mutex_guard);
            refresh_token(session).await?;
        }
    } else {
        debug!("Performing first login");
        drop(mutex_guard);
        login(session).await?;
    }
    Ok(())
}
