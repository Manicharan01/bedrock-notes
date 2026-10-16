use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_SESSIONS: usize = 128;
type DigestKey = [u8; 32];
pub fn digest(value: &str) -> DigestKey {
    Sha256::digest(value.as_bytes()).into()
}

pub struct Auth {
    key: Option<DigestKey>,
    sessions: Mutex<HashMap<DigestKey, Instant>>,
    attempts: Mutex<(Instant, u32)>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    Invalid,
    RateLimited,
    Capacity,
    Internal,
}

impl Auth {
    pub fn new(key: Option<&str>) -> Result<Self, String> {
        if key.is_some_and(|k| {
            k.len() < 32 || k.len() > 256 || !k.is_ascii() || k.chars().any(char::is_control)
        }) {
            return Err("Access keys must contain 32–256 printable ASCII characters".into());
        }
        Ok(Self {
            key: key.map(digest),
            sessions: Mutex::new(HashMap::new()),
            attempts: Mutex::new((Instant::now(), 0)),
        })
    }
    pub fn required(&self) -> bool {
        self.key.is_some()
    }
    pub fn login(&self, key: &str) -> Result<Option<String>, LoginError> {
        let Some(expected) = self.key else {
            return Ok(None);
        };
        let mut attempts = self.attempts.lock().map_err(|_| LoginError::Internal)?;
        if attempts.0.elapsed() >= Duration::from_secs(60) {
            *attempts = (Instant::now(), 0);
        }
        if attempts.1 >= 60 {
            return Err(LoginError::RateLimited);
        }
        attempts.1 += 1;
        drop(attempts);
        if !bool::from(expected.ct_eq(&digest(key))) {
            return Err(LoginError::Invalid);
        }
        let now = Instant::now();
        let mut sessions = self.sessions.lock().map_err(|_| LoginError::Internal)?;
        sessions.retain(|_, expires| *expires > now);
        if sessions.len() >= MAX_SESSIONS {
            return Err(LoginError::Capacity);
        }
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        sessions.insert(digest(&token), now + SESSION_LIFETIME);
        Ok(Some(token))
    }
    pub fn authorized(&self, token: Option<&str>) -> bool {
        if !self.required() {
            return true;
        }
        let Some(token) = token.filter(|t| t.len() == 64) else {
            return false;
        };
        let Ok(mut sessions) = self.sessions.lock() else {
            return false;
        };
        let now = Instant::now();
        sessions.retain(|_, expires| *expires > now);
        sessions.contains_key(&digest(token))
    }
    pub fn logout(&self, token: Option<&str>) {
        if let (Some(token), Ok(mut sessions)) = (token, self.sessions.lock()) {
            sessions.remove(&digest(token));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_sessions_are_removed_and_do_not_authenticate() {
        let auth = Auth::new(Some("synthetic-test-key-that-is-at-least-32-characters")).unwrap();
        let token = auth
            .login("synthetic-test-key-that-is-at-least-32-characters")
            .unwrap()
            .unwrap();
        auth.sessions
            .lock()
            .unwrap()
            .insert(digest(&token), Instant::now() - Duration::from_secs(1));
        assert!(!auth.authorized(Some(&token)));
        assert!(auth.sessions.lock().unwrap().is_empty());
    }
}
