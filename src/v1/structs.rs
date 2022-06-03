use std::error::Error;

use chrono::{DateTime, Local, Utc};
use influxdb::InfluxDbWriteable;

#[derive(Debug)]
pub struct ChargerState {
    pub power: f64,
    pub session: f64,
    pub energy_per_hour: f64,
}

#[derive(Debug)]
pub struct SessionState {
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub lifetime: Option<DateTime<Local>>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState {
            token: None,
            lifetime: None,
            refresh_token: None,
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::new()
    }
}

#[derive(Debug)]
pub enum EaseeError {
    Unathorized,
    LoginFailed,
    HttpFailed,
    InvalidResponse,
    RateLimit,
}

impl std::fmt::Display for EaseeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EaseeError::Unathorized => write!(f, "Unathorized"),
            EaseeError::LoginFailed => write!(f, "Login failed"),
            EaseeError::HttpFailed => write!(f, "Http failed"),
            EaseeError::InvalidResponse => write!(f, "Invalid response"),
            EaseeError::RateLimit => write!(f, "Rate limit"),
        }
    }
}

impl Error for EaseeError {
    fn description(&self) -> &str {
        match *self {
            EaseeError::Unathorized => "Unauthorized",
            EaseeError::LoginFailed => "Login failed",
            EaseeError::HttpFailed => "Http failed",
            EaseeError::InvalidResponse => "Invalid response",
            EaseeError::RateLimit => "Rate limit",
        }
    }
}

#[derive(InfluxDbWriteable)]
pub struct Variable {
    pub time: DateTime<Utc>,
    pub value: f64,
    #[influxdb(tag)]
    pub variable: String,
}
