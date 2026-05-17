use chrono::{DateTime, Utc};

pub fn humanize_since(then: DateTime<Utc>) -> String {
    let now = Utc::now();
    let secs = (now - then).num_seconds();
    if secs < 0 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 48 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn buckets() {
        let now = Utc::now();
        assert!(humanize_since(now + Duration::seconds(5)) == "just now");
        assert!(humanize_since(now - Duration::seconds(5)).ends_with("s ago"));
        assert!(humanize_since(now - Duration::minutes(5)).ends_with("m ago"));
        assert!(humanize_since(now - Duration::hours(5)).ends_with("h ago"));
        assert!(humanize_since(now - Duration::days(5)).ends_with("d ago"));
    }
}
