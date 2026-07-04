//! Secret redaction for platform uploads.
//!
//! Local snapshots capture the full environment (including variable values and
//! the PowerShell profile), which can contain API keys and tokens. Before a
//! snapshot leaves the machine for the platform, we mask values that look
//! secret. This only affects the uploaded copy — local snapshot files are never
//! modified.
//!
//! Detection has two layers: a case-insensitive substring check on the variable
//! *name*, and structural checks on the *value* (token prefixes, key shapes,
//! JWTs, PEM headers) so a secret stored under a benign name is still caught.
//! Both are plain string matching — no `regex` dependency. Keep the value-shape
//! list aligned with the platform's `lib/redact.ts` in Odin-Platform.

use crate::models::environment::EnvironmentSnapshot;

const SECRET_KEYWORDS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PWD",
    "APIKEY",
    "API_KEY",
    "ACCESS_KEY",
    "CREDENTIAL",
    "PRIVATE",
    "AUTH",
    "PAT",
    "KEY",
];

pub const REDACTED: &str = "***redacted***";

/// True when a variable name looks like it holds a secret.
pub fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_KEYWORDS.iter().any(|kw| upper.contains(kw))
}

fn is_b64url(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '='
}

/// True when a value *looks* like a credential regardless of its variable name:
/// GitHub tokens (`ghp_`/`gho_`/…, `github_pat_`), OpenAI/Anthropic-style
/// `sk-…`, AWS access-key ids (`AKIA…`), Slack tokens (`xox[baprs]-…`), JWTs
/// (`eyJ….….…`), and PEM private-key headers. Conservative length bounds keep
/// false positives down.
pub fn is_secret_value(value: &str) -> bool {
    let v = value.trim();

    // GitHub personal-access / OAuth / app tokens.
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        if let Some(rest) = v.strip_prefix(prefix) {
            if rest.len() >= 20 && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return true;
            }
        }
    }
    if let Some(rest) = v.strip_prefix("github_pat_") {
        if rest.len() >= 20 {
            return true;
        }
    }

    // OpenAI / Anthropic-style secret keys.
    if let Some(rest) = v.strip_prefix("sk-") {
        if rest.len() >= 16
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return true;
        }
    }

    // AWS access key id: AKIA followed by 16 uppercase/digit chars.
    if let Some(rest) = v.strip_prefix("AKIA") {
        if rest.len() >= 16
            && rest
                .chars()
                .take(16)
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return true;
        }
    }

    // Slack tokens.
    for prefix in ["xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-"] {
        if v.starts_with(prefix) && v.len() >= prefix.len() + 10 {
            return true;
        }
    }

    // PEM private key block.
    if v.contains("-----BEGIN") && v.contains("PRIVATE KEY") {
        return true;
    }

    // JWT: three base64url segments separated by dots, first starting `eyJ`.
    if v.starts_with("eyJ") {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() == 3
            && parts
                .iter()
                .all(|p| p.len() >= 6 && p.chars().all(is_b64url))
        {
            return true;
        }
    }

    false
}

/// Returns a copy of `env` with secret-looking values masked: any variable
/// whose name matches [`is_secret_name`], and secret-looking assignment lines in
/// the PowerShell profile content.
pub fn redact_environment(mut env: EnvironmentSnapshot) -> EnvironmentSnapshot {
    for var in env
        .user_variables
        .iter_mut()
        .chain(env.machine_variables.iter_mut())
    {
        if is_secret_name(&var.name) || is_secret_value(&var.value) {
            var.value = REDACTED.to_string();
        }
    }

    if let Some(profile) = env.powershell_profile.as_mut() {
        profile.content = redact_profile_content(&profile.content);
    }

    env
}

/// Masks the right-hand side of assignment lines that mention a secret keyword,
/// e.g. `$env:OPENAI_API_KEY = "sk-..."` becomes `$env:OPENAI_API_KEY = ***redacted***`.
fn redact_profile_content(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line_mentions_secret(line) {
                mask_assignment(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_mentions_secret(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    if SECRET_KEYWORDS.iter().any(|kw| upper.contains(kw)) {
        return true;
    }
    // Also mask lines whose value looks secret even without a keyword, e.g.
    // `$env:GH = "ghp_..."`. Check the tokens on the RHS after the first `=`.
    if let Some(idx) = line.find('=') {
        return line[idx + 1..]
            .split(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .any(is_secret_value);
    }
    false
}

fn mask_assignment(line: &str) -> String {
    match line.find('=') {
        Some(idx) => format!("{}= {}", &line[..idx], REDACTED),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::environment::{EnvironmentScope, EnvironmentVariable, ProfileSnapshot};

    fn var(name: &str, value: &str) -> EnvironmentVariable {
        EnvironmentVariable {
            name: name.to_string(),
            value: value.to_string(),
            scope: EnvironmentScope::User,
        }
    }

    #[test]
    fn masks_secret_names_only() {
        let env = EnvironmentSnapshot {
            user_variables: vec![
                var("GITHUB_TOKEN", "ghp_abc"),
                var("EDITOR", "code"),
                var("AWS_SECRET_ACCESS_KEY", "xyz"),
            ],
            machine_variables: vec![],
            path_entries: vec![],
            powershell_profile: None,
            terminal_settings: None,
        };
        let out = redact_environment(env);
        assert_eq!(out.user_variables[0].value, REDACTED);
        assert_eq!(out.user_variables[1].value, "code");
        assert_eq!(out.user_variables[2].value, REDACTED);
    }

    #[test]
    fn masks_secret_by_value_shape_under_benign_name() {
        let env = EnvironmentSnapshot {
            user_variables: vec![
                var("MY_CONFIG", &format!("ghp_{}", "a".repeat(36))),
                var("HOME", "C:\\Users\\ada"),
            ],
            machine_variables: vec![],
            path_entries: vec![],
            powershell_profile: None,
            terminal_settings: None,
        };
        let out = redact_environment(env);
        assert_eq!(out.user_variables[0].value, REDACTED);
        assert_eq!(out.user_variables[1].value, "C:\\Users\\ada");
    }

    #[test]
    fn is_secret_value_positive_cases() {
        assert!(is_secret_value(&format!("ghp_{}", "a".repeat(36))));
        assert!(is_secret_value(&format!("github_pat_{}", "a".repeat(30))));
        assert!(is_secret_value(&format!("sk-{}", "a".repeat(32))));
        assert!(is_secret_value("AKIAABCDEFGHIJKLMNOP"));
        assert!(is_secret_value(&format!("xoxb-{}", "1".repeat(12))));
        assert!(is_secret_value(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.SflKxwRJSMeKKF2QT4fw"
        ));
        assert!(is_secret_value("-----BEGIN RSA PRIVATE KEY-----"));
    }

    #[test]
    fn is_secret_value_negative_cases() {
        assert!(!is_secret_value("code"));
        assert!(!is_secret_value("C:\\Program Files\\Git"));
        assert!(!is_secret_value("ghp_short")); // too short
        assert!(!is_secret_value("sk-123")); // too short
        assert!(!is_secret_value("https://example.com/path"));
    }

    #[test]
    fn masks_secret_profile_lines() {
        let env = EnvironmentSnapshot {
            user_variables: vec![],
            machine_variables: vec![],
            path_entries: vec![],
            powershell_profile: Some(ProfileSnapshot {
                path: "p".into(),
                content: "$env:OPENAI_API_KEY = \"sk-secret\"\nSet-Alias ll ls".into(),
                sha256: "h".into(),
            }),
            terminal_settings: None,
        };
        let out = redact_environment(env);
        let content = out.powershell_profile.unwrap().content;
        assert!(content.contains(REDACTED));
        assert!(!content.contains("sk-secret"));
        assert!(content.contains("Set-Alias ll ls"));
    }
}
