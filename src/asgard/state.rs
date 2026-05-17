use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const RECENT_CAP: usize = 10;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AsgardState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub recent: Vec<RecentEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentEntry {
    pub name: String,
    pub activated_at: DateTime<Utc>,
}

impl AsgardState {
    pub fn record_activation(&mut self, name: &str, when: DateTime<Utc>) {
        self.active_profile = Some(name.to_string());
        self.activated_at = Some(when);
        self.recent.retain(|e| e.name != name);
        self.recent.insert(
            0,
            RecentEntry {
                name: name.to_string(),
                activated_at: when,
            },
        );
        if self.recent.len() > RECENT_CAP {
            self.recent.truncate(RECENT_CAP);
        }
    }

    pub fn clear_active(&mut self) {
        self.active_profile = None;
        self.activated_at = None;
    }

    pub fn drop_profile(&mut self, name: &str) {
        if self.active_profile.as_deref() == Some(name) {
            self.clear_active();
        }
        self.recent.retain(|e| e.name != name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(secs: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn dedup_and_cap() {
        let mut s = AsgardState::default();
        for i in 0..15 {
            s.record_activation(&format!("p{i}"), ts(i));
        }
        assert_eq!(s.recent.len(), RECENT_CAP);
        // newest first
        assert_eq!(s.recent[0].name, "p14");
        assert_eq!(s.active_profile.as_deref(), Some("p14"));

        // re-activating an older one moves it to the front, doesn't dup
        s.record_activation("p10", ts(100));
        assert_eq!(s.recent[0].name, "p10");
        let count = s.recent.iter().filter(|e| e.name == "p10").count();
        assert_eq!(count, 1);
        assert!(s.recent.len() <= RECENT_CAP);
    }

    #[test]
    fn drop_profile_clears_active() {
        let mut s = AsgardState::default();
        s.record_activation("foo", ts(1));
        s.record_activation("bar", ts(2));
        s.drop_profile("bar");
        assert!(s.active_profile.is_none());
        assert!(s.activated_at.is_none());
        assert_eq!(s.recent.len(), 1);
        assert_eq!(s.recent[0].name, "foo");
    }

    #[test]
    fn clear_active_keeps_recent() {
        let mut s = AsgardState::default();
        s.record_activation("foo", ts(1));
        s.clear_active();
        assert!(s.active_profile.is_none());
        assert_eq!(s.recent.len(), 1);
    }
}
