use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub startup_apps: Vec<StartupApp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub browser_urls: Vec<BrowserEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartupApp {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub window: WindowState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<WindowLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum WindowLayout {
    Preset(LayoutPreset),
    TargetedPreset {
        preset: LayoutPreset,
        #[serde(default = "default_monitor")]
        monitor: u32,
    },
    Bounds {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayoutPreset {
    SnapLeft,
    SnapRight,
    TopHalf,
    BottomHalf,
    Quadrant1,
    Quadrant2,
    Quadrant3,
    Quadrant4,
}

fn default_monitor() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
}

/// A browser URL paired with a friendly label. Accepts either form on load:
///
/// ```yaml
/// browser_urls:
///   - https://example.com                    # bare string, name derived from host
///   - { name: docs, url: https://docs.rs }   # explicit name
/// ```
///
/// Always serializes as the structured form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "BrowserEntryRaw")]
pub struct BrowserEntry {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BrowserEntryRaw {
    Bare(String),
    Full {
        #[serde(default)]
        name: Option<String>,
        url: String,
    },
}

impl From<BrowserEntryRaw> for BrowserEntry {
    fn from(r: BrowserEntryRaw) -> Self {
        match r {
            BrowserEntryRaw::Bare(url) => BrowserEntry {
                name: derive_name_from_url(&url),
                url,
            },
            BrowserEntryRaw::Full { name, url } => BrowserEntry {
                name: name
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| derive_name_from_url(&url)),
                url,
            },
        }
    }
}

impl BrowserEntry {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        let url = url.into();
        let raw_name = name.into();
        let name = if raw_name.trim().is_empty() {
            derive_name_from_url(&url)
        } else {
            raw_name
        };
        BrowserEntry { name, url }
    }
}

fn derive_name_from_url(url: &str) -> String {
    let s = url.trim();
    let no_proto = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let host = no_proto.split('/').next().unwrap_or(no_proto);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        "url".to_string()
    } else {
        host.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub name: String,
    pub description: String,
    pub startup_app_count: usize,
    pub browser_url_count: usize,
    pub has_vscode: bool,
}

impl From<&Profile> for ProfileSummary {
    fn from(p: &Profile) -> Self {
        ProfileSummary {
            name: p.name.clone(),
            description: p.description.clone(),
            startup_app_count: p.startup_apps.len(),
            browser_url_count: p.browser_urls.len(),
            has_vscode: p.vscode_workspace.is_some(),
        }
    }
}

pub const RESERVED_NAME: &str = "asgard";

pub fn validate_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("profile name cannot be empty".into());
    }
    if trimmed.eq_ignore_ascii_case(RESERVED_NAME) {
        return Err(format!(
            "`{RESERVED_NAME}` is reserved as the realm name and cannot be used for a profile"
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "profile name may contain only letters, digits, '-' and '_' (no spaces)".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_minimal() {
        let p = Profile {
            name: "x".into(),
            description: String::new(),
            env: BTreeMap::new(),
            startup_apps: vec![],
            vscode_workspace: None,
            browser_urls: vec![],
        };
        let yaml = serde_yaml::to_string(&p).unwrap();
        let back: Profile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(p, back);
        assert!(!yaml.contains("env:"));
        assert!(!yaml.contains("description:"));
        assert!(!yaml.contains("vscode_workspace"));
    }

    #[test]
    fn round_trip_full() {
        let mut env = BTreeMap::new();
        env.insert("RUST_LOG".into(), "debug".into());
        let p = Profile {
            name: "backend-dev".into(),
            description: "Backend".into(),
            env,
            startup_apps: vec![StartupApp {
                name: "editor".into(),
                command: "code".into(),
                args: vec![".".into()],
                cwd: Some("C:\\repos".into()),
                window: WindowState::Maximized,
                layout: None,
            }],
            vscode_workspace: Some("C:\\repos\\backend.code-workspace".into()),
            browser_urls: vec![BrowserEntry::new("docs", "https://example.com")],
        };
        let yaml = serde_yaml::to_string(&p).unwrap();
        let back: Profile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn browser_entry_accepts_bare_string() {
        let yaml = r#"
name: x
browser_urls:
  - https://example.com
  - http://www.foo.bar/path
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(p.browser_urls.len(), 2);
        assert_eq!(p.browser_urls[0].name, "example.com");
        assert_eq!(p.browser_urls[0].url, "https://example.com");
        assert_eq!(p.browser_urls[1].name, "foo.bar");
    }

    #[test]
    fn browser_entry_accepts_structured() {
        let yaml = r#"
name: x
browser_urls:
  - name: docs
    url: https://doc.rust-lang.org
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(p.browser_urls.len(), 1);
        assert_eq!(p.browser_urls[0].name, "docs");
    }

    #[test]
    fn browser_entry_blank_name_falls_back() {
        let raw = BrowserEntryRaw::Full {
            name: Some("   ".into()),
            url: "https://github.com".into(),
        };
        let e: BrowserEntry = raw.into();
        assert_eq!(e.name, "github.com");
    }

    #[test]
    fn derive_name_buckets() {
        assert_eq!(derive_name_from_url("https://example.com"), "example.com");
        assert_eq!(derive_name_from_url("http://www.foo.bar"), "foo.bar");
        assert_eq!(derive_name_from_url("https://a.b.com/x/y"), "a.b.com");
        assert_eq!(derive_name_from_url("not-a-url"), "not-a-url");
        assert_eq!(derive_name_from_url(""), "url");
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("backend-dev").is_ok());
        assert!(validate_name("ai_lab").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(validate_name("Asgard").is_err());
        assert!(validate_name("ASGARD").is_err());
        assert!(validate_name("asgard").is_err());
        assert!(validate_name("has space").is_err());
        assert!(validate_name("has/slash").is_err());
    }

    #[test]
    fn window_state_default() {
        assert_eq!(WindowState::default(), WindowState::Normal);
    }

    #[test]
    fn layout_accepts_legacy_preset() {
        let yaml = r#"
name: x
startup_apps:
  - name: terminal
    command: wt.exe
    layout: SnapRight
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            p.startup_apps[0].layout,
            Some(WindowLayout::Preset(LayoutPreset::SnapRight))
        );
    }

    #[test]
    fn layout_accepts_targeted_preset() {
        let yaml = r#"
name: x
startup_apps:
  - name: terminal
    command: wt.exe
    layout:
      preset: SnapLeft
      monitor: 2
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            p.startup_apps[0].layout,
            Some(WindowLayout::TargetedPreset {
                preset: LayoutPreset::SnapLeft,
                monitor: 2
            })
        );

        let out = serde_yaml::to_string(&p).unwrap();
        let back: Profile = serde_yaml::from_str(&out).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn layout_targeted_preset_defaults_to_primary() {
        let yaml = r#"
name: x
startup_apps:
  - name: terminal
    command: wt.exe
    layout:
      preset: TopHalf
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            p.startup_apps[0].layout,
            Some(WindowLayout::TargetedPreset {
                preset: LayoutPreset::TopHalf,
                monitor: 1
            })
        );
    }

    #[test]
    fn layout_bounds_stay_absolute() {
        let yaml = r#"
name: x
startup_apps:
  - name: terminal
    command: wt.exe
    layout:
      x: 1920
      y: 40
      width: 960
      height: 1000
"#;
        let p: Profile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            p.startup_apps[0].layout,
            Some(WindowLayout::Bounds {
                x: 1920,
                y: 40,
                width: 960,
                height: 1000
            })
        );
    }
}
