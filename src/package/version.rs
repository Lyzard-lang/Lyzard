use std::fmt;

/// A parsed semantic version: MAJOR.MINOR.PATCH.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self { major, minor, patch }
    }

    /// Parse "MAJOR.MINOR.PATCH". Partial versions are allowed: "2.0" means
    /// 2.0.0 and "3" means 3.0.0, matching SemVer's trailing-zero rule.
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(VersionError::new("empty version string".to_string()));
        }
        let parts: Vec<&str> = trimmed.split('.').collect();
        if parts.len() > 3 {
            return Err(VersionError::new(format!(
                "too many components in version `{trimmed}`"
            )));
        }
        let numbers: Vec<u64> = parts
            .iter()
            .map(|p| {
                p.parse::<u64>().map_err(|_| {
                    VersionError::new(format!("invalid component `{p}` in version `{trimmed}`"))
                })
            })
            .collect::<Result<_, _>>()?;
        let mut it = numbers.into_iter();
        Ok(Version {
            major: it.next().unwrap_or(0),
            minor: it.next().unwrap_or(0),
            patch: it.next().unwrap_or(0),
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Error produced when a version or requirement string can't be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionError {
    message: String,
}

impl VersionError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid version: {}", self.message)
    }
}

impl std::error::Error for VersionError {}

/// A single comparator: an operator + a version to compare against
#[derive(Debug, Clone, PartialEq)]
struct Comparator {
    op: CompareOp,
    version: Version,
}

#[derive(Debug, Clone, PartialEq)]
enum CompareOp {
    Exact,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl Comparator {
    fn matches(&self, v: &Version) -> bool {
        match self.op {
            CompareOp::Exact => v == &self.version,
            CompareOp::Gt => v > &self.version,
            CompareOp::Gte => v >= &self.version,
            CompareOp::Lt => v < &self.version,
            CompareOp::Lte => v <= &self.version,
        }
    }
}

/// A version requirement — one or more comparators ANDed together,
/// OR the special caret/tilde/wildcard shorthand forms
#[derive(Debug, Clone, PartialEq)]
pub struct VersionReq {
    comparators: Vec<Comparator>,
}

impl VersionReq {
    /// Parse a requirement string. Supports:
    ///   "1.2.3"        -> exact match
    ///   "^1.2.3"       -> caret (compatible) range
    ///   "~1.2.3"       -> tilde (patch-only) range
    ///   ">=1.0, <2.0"  -> explicit comma-separated comparator chain
    ///   "*"            -> matches anything
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let s = s.trim();

        if s == "*" {
            return Ok(VersionReq { comparators: vec![] }); // empty = matches all
        }

        if let Some(rest) = s.strip_prefix('^') {
            let v = Version::parse(rest)?;
            return Ok(Self::caret_range(&v));
        }

        if let Some(rest) = s.strip_prefix('~') {
            let v = Version::parse(rest)?;
            return Ok(Self::tilde_range(&v));
        }

        if s.contains(',') {
            let mut comparators = Vec::new();
            for part in s.split(',') {
                comparators.push(Self::parse_single_comparator(part.trim())?);
            }
            return Ok(VersionReq { comparators });
        }

        // Single comparator, or bare "1.2.3" treated as exact
        let comparator = Self::parse_single_comparator(s)?;
        Ok(VersionReq {
            comparators: vec![comparator],
        })
    }

    fn parse_single_comparator(s: &str) -> Result<Comparator, VersionError> {
        let (op, rest) = if let Some(r) = s.strip_prefix(">=") {
            (CompareOp::Gte, r)
        } else if let Some(r) = s.strip_prefix("<=") {
            (CompareOp::Lte, r)
        } else if let Some(r) = s.strip_prefix('>') {
            (CompareOp::Gt, r)
        } else if let Some(r) = s.strip_prefix('<') {
            (CompareOp::Lt, r)
        } else if let Some(r) = s.strip_prefix('=') {
            (CompareOp::Exact, r)
        } else {
            (CompareOp::Exact, s)
        };

        let version = Version::parse(rest.trim())?;
        Ok(Comparator { op, version })
    }

    /// Build a caret range: ^1.2.3 -> >=1.2.3, <2.0.0
    /// Special-cased for 0.x versions per the SemVer/Cargo convention
    fn caret_range(v: &Version) -> VersionReq {
        let upper = if v.major > 0 {
            Version::new(v.major + 1, 0, 0)
        } else if v.minor > 0 {
            Version::new(0, v.minor + 1, 0)
        } else {
            Version::new(0, 0, v.patch + 1)
        };
        VersionReq {
            comparators: vec![
                Comparator {
                    op: CompareOp::Gte,
                    version: v.clone(),
                },
                Comparator {
                    op: CompareOp::Lt,
                    version: upper,
                },
            ],
        }
    }

    /// Build a tilde range: ~1.2.3 -> >=1.2.3, <1.3.0
    fn tilde_range(v: &Version) -> VersionReq {
        let upper = Version::new(v.major, v.minor + 1, 0);
        VersionReq {
            comparators: vec![
                Comparator {
                    op: CompareOp::Gte,
                    version: v.clone(),
                },
                Comparator {
                    op: CompareOp::Lt,
                    version: upper,
                },
            ],
        }
    }

    /// Does the given version satisfy ALL comparators in this requirement?
    pub fn matches(&self, v: &Version) -> bool {
        self.comparators.iter().all(|c| c.matches(v))
    }

    /// Given a list of available versions, return the HIGHEST one that
    /// satisfies this requirement (the resolver's core operation)
    pub fn best_match<'a>(&self, available: &'a [Version]) -> Option<&'a Version> {
        available
            .iter()
            .filter(|v| self.matches(v))
            .max()
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.comparators.is_empty() {
            return write!(f, "*");
        }
        let parts: Vec<String> = self
            .comparators
            .iter()
            .map(|c| {
                let op_str = match c.op {
                    CompareOp::Exact => "=",
                    CompareOp::Gt => ">",
                    CompareOp::Gte => ">=",
                    CompareOp::Lt => "<",
                    CompareOp::Lte => "<=",
                };
                format!("{}{}", op_str, c.version)
            })
            .collect();
        write!(f, "{}", parts.join(", "))
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn test_parse_full() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));
    }

    #[test]
    fn test_parse_partial_components_default_to_zero() {
        assert_eq!(Version::parse("2.0").unwrap(), Version::new(2, 0, 0));
        assert_eq!(Version::parse("3").unwrap(), Version::new(3, 0, 0));
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        assert_eq!(Version::parse(" 1.2.3 ").unwrap(), Version::new(1, 2, 3));
    }

    #[test]
    fn test_parse_rejects_invalid() {
        assert!(Version::parse("").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("abc").is_err());
        assert!(Version::parse("1.2.x").is_err());
    }

    #[test]
    fn test_ordering() {
        assert!(Version::new(1, 2, 3) < Version::new(1, 2, 4));
        assert!(Version::new(1, 2, 4) < Version::new(1, 3, 0));
        assert!(Version::new(1, 3, 0) < Version::new(2, 0, 0));
        assert_eq!(
            Version::new(1, 2, 3).cmp(&Version::new(1, 2, 3)),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
        assert_eq!(Version::parse("0.0.0").unwrap().to_string(), "0.0.0");
    }
}

#[cfg(test)]
mod version_req_tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn test_exact_match() {
        let r = VersionReq::parse("1.2.3").unwrap();
        assert!(r.matches(&v("1.2.3")));
        assert!(!r.matches(&v("1.2.4")));
    }
    #[test]
    fn test_wildcard_matches_all() {
        let r = VersionReq::parse("*").unwrap();
        assert!(r.matches(&v("0.0.1")));
        assert!(r.matches(&v("99.99.99")));
    }

    #[test]
    fn test_caret_matches_minor_bump() {
        let r = VersionReq::parse("^1.2.3").unwrap();
        assert!(r.matches(&v("1.9.0")));
    }
    #[test]
    fn test_caret_matches_patch_bump() {
        let r = VersionReq::parse("^1.2.3").unwrap();
        assert!(r.matches(&v("1.2.9")));
    }
    #[test]
    fn test_caret_rejects_major_bump() {
        let r = VersionReq::parse("^1.2.3").unwrap();
        assert!(!r.matches(&v("2.0.0")));
    }
    #[test]
    fn test_caret_rejects_lower() {
        let r = VersionReq::parse("^1.2.3").unwrap();
        assert!(!r.matches(&v("1.2.2")));
    }

    #[test]
    fn test_caret_zero_major_treats_minor_as_breaking() {
        let r = VersionReq::parse("^0.2.3").unwrap();
        assert!(r.matches(&v("0.2.9")));
        assert!(!r.matches(&v("0.3.0")));
    }
    #[test]
    fn test_caret_zero_zero_only_patch_safe() {
        let r = VersionReq::parse("^0.0.3").unwrap();
        assert!(r.matches(&v("0.0.3")));
        assert!(!r.matches(&v("0.0.4")));
        assert!(!r.matches(&v("0.1.0")));
    }

    #[test]
    fn test_tilde_matches_patch_only() {
        let r = VersionReq::parse("~1.2.3").unwrap();
        assert!(r.matches(&v("1.2.9")));
        assert!(!r.matches(&v("1.3.0")));
    }

    #[test]
    fn test_explicit_range() {
        let r = VersionReq::parse(">=1.0.0, <2.0.0").unwrap();
        assert!(r.matches(&v("1.5.0")));
        assert!(!r.matches(&v("2.0.0")));
        assert!(!r.matches(&v("0.9.0")));
    }

    #[test]
    fn test_greater_than() {
        let r = VersionReq::parse(">1.0.0").unwrap();
        assert!(r.matches(&v("1.0.1")));
        assert!(!r.matches(&v("1.0.0")));
    }
    #[test]
    fn test_less_than_or_equal() {
        let r = VersionReq::parse("<=1.0.0").unwrap();
        assert!(r.matches(&v("1.0.0")));
        assert!(!r.matches(&v("1.0.1")));
    }

    #[test]
    fn test_best_match_picks_highest() {
        let r = VersionReq::parse("^1.0.0").unwrap();
        let available = vec![v("1.0.0"), v("1.5.0"), v("1.2.0"), v("2.0.0")];
        assert_eq!(r.best_match(&available), Some(&v("1.5.0")));
    }

    #[test]
    fn test_best_match_none_when_incompatible() {
        let r = VersionReq::parse("^3.0.0").unwrap();
        let available = vec![v("1.0.0"), v("2.0.0")];
        assert_eq!(r.best_match(&available), None);
    }

    #[test]
    fn test_version_req_display() {
        assert_eq!(VersionReq::parse("*").unwrap().to_string(), "*");
        assert_eq!(
            VersionReq::parse(">=1.0.0, <2.0.0").unwrap().to_string(),
            ">=1.0.0, <2.0.0"
        );
        assert_eq!(VersionReq::parse("1.2.3").unwrap().to_string(), "=1.2.3");
    }
}
