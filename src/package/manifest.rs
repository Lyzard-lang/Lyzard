use std::collections::HashMap;

use super::version::{Version, VersionReq};

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: Version,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub dependencies: HashMap<String, VersionReq>,
    pub dev_dependencies: HashMap<String, VersionReq>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManifestError {
    MissingSection(String),
    MissingField(String, String), // section, field
    InvalidValue(String, String), // field, reason
    ParseError(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSection(s) => write!(f, "lyz.toml is missing required section [{}]", s),
            Self::MissingField(s, k) => {
                write!(f, "lyz.toml [{}] is missing required field '{}'", s, k)
            }
            Self::InvalidValue(k, r) => write!(f, "lyz.toml field '{}' is invalid: {}", k, r),
            Self::ParseError(msg) => write!(f, "lyz.toml parse error: {}", msg),
        }
    }
}
impl std::error::Error for ManifestError {}

impl Manifest {
    pub fn parse(toml_source: &str) -> Result<Self, ManifestError> {
        let sections = parse_toml_sections(toml_source)?;

        let package = sections
            .get("package")
            .ok_or_else(|| ManifestError::MissingSection("package".to_string()))?;

        let name = package
            .get("name")
            .ok_or_else(|| ManifestError::MissingField("package".to_string(), "name".to_string()))?
            .clone();

        let version_str = package
            .get("version")
            .ok_or_else(|| ManifestError::MissingField("package".to_string(), "version".to_string()))?;
        let version = Version::parse(version_str)
            .map_err(|e| ManifestError::InvalidValue("version".to_string(), e.to_string()))?;

        let authors = package
            .get("authors")
            .map(|s| parse_toml_array(s))
            .unwrap_or_default();

        let license = package.get("license").cloned();

        let dependencies = sections
            .get("dependencies")
            .map(parse_dependency_table)
            .transpose()?
            .unwrap_or_default();

        let dev_dependencies = sections
            .get("dev-dependencies")
            .map(parse_dependency_table)
            .transpose()?
            .unwrap_or_default();

        Ok(Manifest {
            name,
            version,
            authors,
            license,
            dependencies,
            dev_dependencies,
        })
    }

    /// Serialize back to lyz.toml format (used by `lyzard init` / `lyzard add`)
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("version = \"{}\"\n", self.version));
        if !self.authors.is_empty() {
            let authors_str = self
                .authors
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("authors = [{}]\n", authors_str));
        }
        if let Some(lic) = &self.license {
            out.push_str(&format!("license = \"{}\"\n", lic));
        }

        if !self.dependencies.is_empty() {
            out.push_str("\n[dependencies]\n");
            let mut deps: Vec<_> = self.dependencies.iter().collect();
            deps.sort_by_key(|(k, _)| *k);
            for (name, req) in deps {
                out.push_str(&format!("{} = \"{}\"\n", name, req));
            }
        }

        out
    }
}

/// Minimal TOML parser: splits into [section] -> { key: raw_value_string }
/// Handles: comments (#), quoted strings, bare arrays like ["a", "b"]
fn parse_toml_sections(source: &str) -> Result<HashMap<String, HashMap<String, String>>, ManifestError> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for raw_line in source.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            sections.entry(current_section.clone()).or_default();
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim();
            let cleaned = strip_quotes(value);
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key, cleaned);
        } else if !current_section.is_empty() {
            return Err(ManifestError::ParseError(format!("malformed line: '{}'", line)));
        }
    }

    Ok(sections)
}

fn strip_comment(line: &str) -> &str {
    // Naive: does not handle '#' inside quoted strings — acceptable for MVP scope
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_toml_array(s: &str) -> Vec<String> {
    // Expects raw form like: ["Ahmed", "Sara"]  (already had outer quotes
    // stripped by strip_quotes if it was a plain string, so re-check brackets)
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        s[1..s.len() - 1]
            .split(',')
            .map(|item| strip_quotes(item.trim()))
            .filter(|item| !item.is_empty())
            .collect()
    } else if !s.is_empty() {
        vec![s.to_string()]
    } else {
        vec![]
    }
}

fn parse_dependency_table(table: &HashMap<String, String>) -> Result<HashMap<String, VersionReq>, ManifestError> {
    let mut deps = HashMap::new();
    for (name, version_str) in table {
        let req = VersionReq::parse(version_str)
            .map_err(|e| ManifestError::InvalidValue(name.clone(), e.to_string()))?;
        deps.insert(name.clone(), req);
    }
    Ok(deps)
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[package]
name = "my_app"
version = "0.1.0"
"#;
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.name, "my_app");
        assert_eq!(m.version, Version::new(0, 1, 0));
    }

    #[test]
    fn test_parse_missing_package_section_errors() {
        let toml = "[dependencies]\nfoo = \"1.0.0\"";
        assert!(matches!(
            Manifest::parse(toml),
            Err(ManifestError::MissingSection(_))
        ));
    }

    #[test]
    fn test_parse_missing_name_errors() {
        let toml = "[package]\nversion = \"1.0.0\"";
        assert!(matches!(
            Manifest::parse(toml),
            Err(ManifestError::MissingField(_, _))
        ));
    }

    #[test]
    fn test_parse_invalid_version_errors() {
        let toml = "[package]\nname = \"x\"\nversion = \"not-a-version\"";
        assert!(matches!(
            Manifest::parse(toml),
            Err(ManifestError::InvalidValue(_, _))
        ));
    }

    #[test]
    fn test_parse_authors_array() {
        let toml = r#"
[package]
name = "x"
version = "1.0.0"
authors = ["Ahmed", "Sara"]
"#;
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.authors, vec!["Ahmed".to_string(), "Sara".to_string()]);
    }

    #[test]
    fn test_parse_dependencies() {
        let toml = r#"
[package]
name = "x"
version = "1.0.0"

[dependencies]
http_client = "1.2.0"
json = "^0.8.0"
"#;
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.contains_key("http_client"));
        assert!(m.dependencies.contains_key("json"));
    }

    #[test]
    fn test_parse_dev_dependencies_separate() {
        let toml = r#"
[package]
name = "x"
version = "1.0.0"

[dependencies]
a = "1.0.0"

[dev-dependencies]
test_framework = "0.3.0"
"#;
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dev_dependencies.len(), 1);
        assert!(!m.dependencies.contains_key("test_framework"));
    }

    #[test]
    fn test_parse_ignores_comments() {
        let toml = "# this is a comment\n[package]\nname = \"x\" # inline note\nversion = \"1.0.0\"";
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.name, "x");
    }

    #[test]
    fn test_parse_license_optional() {
        let toml = "[package]\nname = \"x\"\nversion = \"1.0.0\"";
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.license, None);
    }

    #[test]
    fn test_to_toml_roundtrip() {
        let toml = r#"
[package]
name = "roundtrip_test"
version = "2.3.4"

[dependencies]
foo = "^1.0.0"
"#;
        let m = Manifest::parse(toml).unwrap();
        let regenerated = m.to_toml();
        let m2 = Manifest::parse(&regenerated).unwrap();
        assert_eq!(m, m2);
    }
}
