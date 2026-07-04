//! Secret redaction for platform uploads.
//!
//! Local snapshots capture the full environment (including variable values and
//! the PowerShell profile), which can contain API keys and tokens. Before a
//! snapshot leaves the machine for the platform, we mask values that look
//! secret. This only affects the uploaded copy — local snapshot files are never
//! modified. Matching is a case-insensitive substring check on the variable
//! name, so no `regex` dependency is needed.

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

/// Returns a copy of `env` with secret-looking values masked: any variable
/// whose name matches [`is_secret_name`], and secret-looking assignment lines in
/// the PowerShell profile content.
pub fn redact_environment(mut env: EnvironmentSnapshot) -> EnvironmentSnapshot {
    for var in env
        .user_variables
        .iter_mut()
        .chain(env.machine_variables.iter_mut())
    {
        if is_secret_name(&var.name) {
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
    SECRET_KEYWORDS.iter().any(|kw| upper.contains(kw))
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
