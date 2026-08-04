use std::collections::{HashMap, VecDeque};

use super::version::{Version, VersionReq};

/// A minimal in-memory view of "what versions exist and what do they depend on"
/// In Task 1105 this gets backed by a real HTTP registry client;
/// for resolver logic and testing we keep it as a simple trait.
pub trait PackageIndex {
    /// All published versions of a package, in any order
    fn available_versions(&self, name: &str) -> Vec<Version>;
    /// The dependencies of one specific published version
    fn dependencies_of(&self, name: &str, version: &Version) -> HashMap<String, VersionReq>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    /// No version of `name` exists that satisfies ALL accumulated requirements
    Conflict {
        name: String,
        requirements: Vec<String>, // human-readable list of the conflicting requirements
    },
    /// The package doesn't exist in the index at all
    NotFound(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict { name, requirements } => write!(
                f,
                "could not find a version of '{}' satisfying all requirements: {}",
                name,
                requirements.join(", ")
            ),
            Self::NotFound(name) => write!(f, "package '{}' was not found in the registry", name),
        }
    }
}
impl std::error::Error for ResolveError {}

pub struct Resolver<'a> {
    index: &'a dyn PackageIndex,
    /// Accumulated version requirements per package, collected as we walk the tree
    requirements: HashMap<String, Vec<VersionReq>>,
    /// The final chosen version per package
    resolved: HashMap<String, Version>,
}

impl<'a> Resolver<'a> {
    pub fn new(index: &'a dyn PackageIndex) -> Self {
        Resolver {
            index,
            requirements: HashMap::new(),
            resolved: HashMap::new(),
        }
    }

    /// Resolve a full dependency tree starting from the given root dependencies.
    /// Returns the flat, deduplicated set of exact versions to install.
    pub fn resolve(
        mut self,
        root_deps: &HashMap<String, VersionReq>,
    ) -> Result<Vec<ResolvedPackage>, ResolveError> {
        let mut frontier: VecDeque<(String, VersionReq)> = root_deps
            .iter()
            .map(|(n, r)| (n.clone(), r.clone()))
            .collect();

        while let Some((name, req)) = frontier.pop_front() {
            self.requirements.entry(name.clone()).or_default().push(req);

            let available = self.index.available_versions(&name);
            if available.is_empty() {
                return Err(ResolveError::NotFound(name));
            }

            let chosen = self.pick_best_version(&name, &available)?;

            // If we already resolved this package at a DIFFERENT version,
            // re-check that the new requirement is compatible with the
            // existing choice before accepting it (simple re-validation —
            // full backtracking is out of scope for this MVP resolver)
            if let Some(existing) = self.resolved.get(&name) {
                if existing == &chosen {
                    continue; // already resolved consistently, nothing new to explore
                }
                // Version changed due to a new tighter constraint — re-verify
                // ALL accumulated requirements still hold for the new pick
                let all_reqs = self.requirements.get(&name).cloned().unwrap_or_default();
                if !all_reqs.iter().all(|r| r.matches(&chosen)) {
                    return Err(ResolveError::Conflict {
                        name: name.clone(),
                        requirements: all_reqs.iter().map(|r| r.to_string()).collect(),
                    });
                }
            }

            self.resolved.insert(name.clone(), chosen.clone());

            // Queue this package's own dependencies for resolution
            for (dep_name, dep_req) in self.index.dependencies_of(&name, &chosen) {
                frontier.push_back((dep_name, dep_req));
            }
        }

        let mut result: Vec<ResolvedPackage> = self
            .resolved
            .into_iter()
            .map(|(name, version)| ResolvedPackage { name, version })
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Among all requirements accumulated so far for `name`, find the
    /// highest available version satisfying ALL of them simultaneously
    fn pick_best_version(&self, name: &str, available: &[Version]) -> Result<Version, ResolveError> {
        let reqs = self.requirements.get(name).cloned().unwrap_or_default();

        let candidate = available
            .iter()
            .filter(|v| reqs.iter().all(|r| r.matches(v)))
            .max()
            .cloned();

        candidate.ok_or_else(|| ResolveError::Conflict {
            name: name.to_string(),
            requirements: reqs.iter().map(|r| r.to_string()).collect(),
        })
    }
}

#[cfg(test)]
mod resolver_tests {
    use super::*;
    use std::collections::HashMap;

    /// A fake in-memory package index for testing, built as a simple map
    type PackageEntries = Vec<(Version, HashMap<String, VersionReq>)>;

    struct FakeIndex {
        packages: HashMap<String, PackageEntries>,
    }

    impl FakeIndex {
        fn new() -> Self {
            FakeIndex {
                packages: HashMap::new(),
            }
        }

        fn add(mut self, name: &str, version: &str, deps: Vec<(&str, &str)>) -> Self {
            let v = Version::parse(version).unwrap();
            let dep_map: HashMap<String, VersionReq> = deps
                .into_iter()
                .map(|(n, r)| (n.to_string(), VersionReq::parse(r).unwrap()))
                .collect();
            self.packages.entry(name.to_string()).or_default().push((v, dep_map));
            self
        }
    }

    impl PackageIndex for FakeIndex {
        fn available_versions(&self, name: &str) -> Vec<Version> {
            self.packages
                .get(name)
                .map(|versions| versions.iter().map(|(v, _)| v.clone()).collect())
                .unwrap_or_default()
        }
        fn dependencies_of(&self, name: &str, version: &Version) -> HashMap<String, VersionReq> {
            self.packages
                .get(name)
                .and_then(|versions| versions.iter().find(|(v, _)| v == version))
                .map(|(_, deps)| deps.clone())
                .unwrap_or_default()
        }
    }

    #[test]
    fn test_resolve_single_dependency() {
        let index = FakeIndex::new().add("http_client", "1.2.0", vec![]);
        let mut root = HashMap::new();
        root.insert("http_client".to_string(), VersionReq::parse("^1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "http_client");
        assert_eq!(result[0].version, Version::new(1, 2, 0));
    }

    #[test]
    fn test_resolve_picks_highest_satisfying_version() {
        let index = FakeIndex::new()
            .add("json", "1.0.0", vec![])
            .add("json", "1.5.0", vec![])
            .add("json", "2.0.0", vec![]); // excluded by ^1.0.0
        let mut root = HashMap::new();
        root.insert("json".to_string(), VersionReq::parse("^1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root).unwrap();
        assert_eq!(result[0].version, Version::new(1, 5, 0));
    }

    #[test]
    fn test_resolve_transitive_dependency() {
        let index = FakeIndex::new()
            .add("http_client", "1.0.0", vec![("json", "^1.0.0")])
            .add("json", "1.2.0", vec![]);
        let mut root = HashMap::new();
        root.insert("http_client".to_string(), VersionReq::parse("^1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root).unwrap();
        assert_eq!(result.len(), 2); // http_client AND its transitive dep json
        assert!(result
            .iter()
            .any(|p| p.name == "json" && p.version == Version::new(1, 2, 0)));
    }

    #[test]
    fn test_diamond_dependency_resolves_to_compatible_version() {
        // my_app -> http_client ^1.0.0 -> json ^1.0.0
        // my_app -> json_pretty ^2.0.0 -> json ^1.5.0
        // Both constraints must be satisfied by ONE json version
        let index = FakeIndex::new()
            .add("http_client", "1.0.0", vec![("json", "^1.0.0")])
            .add("json_pretty", "2.0.0", vec![("json", "^1.5.0")])
            .add("json", "1.4.0", vec![])
            .add("json", "1.6.0", vec![]); // satisfies BOTH ^1.0.0 and ^1.5.0

        let mut root = HashMap::new();
        root.insert("http_client".to_string(), VersionReq::parse("^1.0.0").unwrap());
        root.insert("json_pretty".to_string(), VersionReq::parse("^2.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root).unwrap();
        let json_pkg = result.iter().find(|p| p.name == "json").unwrap();
        assert_eq!(json_pkg.version, Version::new(1, 6, 0));
    }

    #[test]
    fn test_missing_package_errors() {
        let index = FakeIndex::new();
        let mut root = HashMap::new();
        root.insert("nonexistent".to_string(), VersionReq::parse("1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root);
        assert!(matches!(result, Err(ResolveError::NotFound(_))));
    }

    #[test]
    fn test_unsatisfiable_requirement_errors() {
        let index = FakeIndex::new().add("foo", "1.0.0", vec![]);
        let mut root = HashMap::new();
        root.insert("foo".to_string(), VersionReq::parse("^2.0.0").unwrap()); // no 2.x exists

        let result = Resolver::new(&index).resolve(&root);
        assert!(matches!(result, Err(ResolveError::Conflict { .. })));
    }

    #[test]
    fn test_resolve_deduplicates_shared_dependency() {
        // Two different top-level deps both requiring the SAME dep at compatible versions
        let index = FakeIndex::new()
            .add("a", "1.0.0", vec![("shared", "^1.0.0")])
            .add("b", "1.0.0", vec![("shared", "^1.0.0")])
            .add("shared", "1.0.0", vec![]);
        let mut root = HashMap::new();
        root.insert("a".to_string(), VersionReq::parse("^1.0.0").unwrap());
        root.insert("b".to_string(), VersionReq::parse("^1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root).unwrap();
        let shared_count = result.iter().filter(|p| p.name == "shared").count();
        assert_eq!(shared_count, 1, "shared dependency should only appear ONCE in the resolved set");
    }

    #[test]
    fn test_incompatible_transitive_requirements_error() {
        // a -> shared ^1.0.0 and b -> shared ^2.0.0 — no single version of
        // `shared` can satisfy both, so resolution must fail with a conflict
        let index = FakeIndex::new()
            .add("a", "1.0.0", vec![("shared", "^1.0.0")])
            .add("b", "1.0.0", vec![("shared", "^2.0.0")])
            .add("shared", "1.5.0", vec![])
            .add("shared", "2.5.0", vec![]);
        let mut root = HashMap::new();
        root.insert("a".to_string(), VersionReq::parse("^1.0.0").unwrap());
        root.insert("b".to_string(), VersionReq::parse("^1.0.0").unwrap());

        let result = Resolver::new(&index).resolve(&root);
        assert!(matches!(result, Err(ResolveError::Conflict { .. })));
    }
}
