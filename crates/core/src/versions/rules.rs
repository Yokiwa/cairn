use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsName {
    Windows,
    Osx,
    Linux,
    #[serde(other)]
    Unknown,
}

impl OsName {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            OsName::Windows
        } else if cfg!(target_os = "macos") {
            OsName::Osx
        } else {
            OsName::Linux
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            OsName::Windows => "windows",
            OsName::Osx => "osx",
            OsName::Linux => "linux",
            OsName::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<OsName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

/// What rules are evaluated against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub os: OsName,
    /// "x86_64", "arm64" or "x86".
    pub arch: String,
    pub os_version: String,
    pub features: HashMap<String, bool>,
}

impl Environment {
    pub fn current() -> Self {
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "arm64",
            "x86" => "x86",
            other => other,
        };
        Environment {
            os: OsName::current(),
            arch: arch.into(),
            os_version: String::new(),
            features: HashMap::new(),
        }
    }
}

fn matches(rule: &Rule, env: &Environment) -> bool {
    if let Some(os) = &rule.os {
        if os.name.is_some_and(|n| n != env.os) {
            return false;
        }
        if let Some(arch) = &os.arch {
            if arch != &env.arch {
                return false;
            }
        }
        if let Some(v) = &os.version {
            match regex::Regex::new(v) {
                Ok(re) if re.is_match(&env.os_version) => {}
                _ => return false,
            }
        }
    }
    if let Some(features) = &rule.features {
        for (k, want) in features {
            if env.features.get(k).copied().unwrap_or(false) != *want {
                return false;
            }
        }
    }
    true
}

/// Mojang semantics: no rules → allowed; otherwise start disallowed and let
/// the last matching rule decide.
pub fn rules_allow(rules: &[Rule], env: &Environment) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if matches(r, env) {
            allowed = r.action == Action::Allow;
        }
    }
    allowed
}
