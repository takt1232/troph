use axum::http::{header, HeaderMap};
use rand::RngCore;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

pub const SESSION_COOKIE: &str = "pf_session";
pub const CSRF_HEADER: &str = "x-csrf-token";
const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_ATTEMPTS: usize = 8;

pub struct Auth {
    pin: String,
    csrf: String,
    sessions: RwLock<HashMap<String, Instant>>,
    attempts: Mutex<HashMap<IpAddr, Vec<Instant>>>,
}

#[derive(Clone, Debug)]
pub struct AuthSnapshot {
    csrf: String,
    sessions: Vec<(String, Instant)>,
}

impl Auth {
    pub fn new(pin: String) -> Self {
        Self {
            pin,
            csrf: random_hex(),
            sessions: RwLock::new(HashMap::new()),
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_snapshot(pin: String, snapshot: AuthSnapshot) -> Self {
        Self {
            pin,
            csrf: snapshot.csrf,
            sessions: RwLock::new(snapshot.sessions.into_iter().collect()),
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn snapshot(&self) -> AuthSnapshot {
        let now = Instant::now();
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        sessions.retain(|_, seen| now.duration_since(*seen) < SESSION_TTL);
        AuthSnapshot {
            csrf: self.csrf.clone(),
            sessions: sessions
                .iter()
                .map(|(token, seen)| (token.clone(), *seen))
                .collect(),
        }
    }

    pub fn csrf_token(&self) -> &str {
        &self.csrf
    }

    pub fn pin_matches(&self, candidate: &str) -> bool {
        self.pin.as_bytes().ct_eq(candidate.as_bytes()).unwrap_u8() == 1
    }

    pub fn create_session(&self) -> String {
        let token = random_hex();
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        sessions.retain(|_, seen| now.duration_since(*seen) < SESSION_TTL);
        sessions.insert(token.clone(), now);
        token
    }

    pub fn session_token(&self, headers: &HeaderMap) -> Option<String> {
        let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
        for part in cookie.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    pub fn is_authenticated(&self, headers: &HeaderMap) -> bool {
        let Some(token) = self.session_token(headers) else {
            return false;
        };
        let now = Instant::now();
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        match sessions.get(&token) {
            Some(seen) if now.duration_since(*seen) < SESSION_TTL => {
                sessions.insert(token, now);
                true
            }
            Some(_) => {
                sessions.remove(&token);
                false
            }
            None => false,
        }
    }

    pub fn remove_session(&self, headers: &HeaderMap) {
        if let Some(token) = self.session_token(headers) {
            self.sessions
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&token);
        }
    }

    pub fn check_csrf_header(&self, headers: &HeaderMap) -> bool {
        headers
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| self.check_csrf_value(value))
            .unwrap_or(false)
    }

    pub fn check_csrf_value(&self, value: &str) -> bool {
        self.csrf.as_bytes().ct_eq(value.as_bytes()).unwrap_u8() == 1
    }

    pub fn rate_limited(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        let entries = attempts.entry(ip).or_default();
        entries.retain(|seen| now.duration_since(*seen) < ATTEMPT_WINDOW);
        entries.len() >= MAX_ATTEMPTS
    }

    pub fn record_attempt(&self, ip: IpAddr) {
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        attempts.entry(ip).or_default().push(Instant::now());
    }

    pub fn clear_attempts(&self, ip: IpAddr) {
        self.attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&ip);
    }
}

pub fn session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        SESSION_TTL.as_secs()
    )
}

pub fn clear_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
}

fn random_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_and_csrf_compare() {
        let auth = Auth::new("123456".to_string());
        assert!(auth.pin_matches("123456"));
        assert!(!auth.pin_matches("123457"));
        assert!(!auth.pin_matches("12345"));
        let csrf = auth.csrf_token().to_string();
        assert!(auth.check_csrf_value(&csrf));
        assert!(!auth.check_csrf_value("nope"));
    }

    #[test]
    fn snapshot_preserves_sessions_and_csrf() {
        let auth = Auth::new("123456".to_string());
        let token = auth.create_session();
        let csrf = auth.csrf_token().to_string();
        let snapshot = auth.snapshot();
        let restored = Auth::with_snapshot("123456".to_string(), snapshot);
        assert_eq!(restored.csrf_token(), csrf);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            axum::http::HeaderValue::from_str(&session_cookie(&token)).unwrap(),
        );
        assert!(restored.is_authenticated(&headers));
        assert!(restored.check_csrf_value(&csrf));
        assert!(restored.pin_matches("123456"));
    }
}
