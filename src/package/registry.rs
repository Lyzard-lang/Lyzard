use std::collections::HashMap;

use super::version::{Version, VersionReq};

/// Metadata for one published package version, as returned by the registry
#[derive(Debug, Clone, PartialEq)]
pub struct PackageMetadata {
    pub name: String,
    pub version: Version,
    pub dependencies: HashMap<String, VersionReq>,
    /// URL (or, for the fake client, an in-memory key) to fetch the source archive
    pub download_url: String,
}

/// Abstraction over "wherever package data comes from" — a real HTTP
/// registry, or (for tests / offline dev) an in-memory fake.
pub trait RegistryClient {
    /// List all published versions of a package (empty if package unknown)
    fn list_versions(&self, name: &str) -> Vec<Version>;

    /// Fetch full metadata for one specific published version
    fn get_metadata(&self, name: &str, version: &Version) -> Option<PackageMetadata>;

    /// Download the package's source files (returns (relative_path, content) pairs)
    fn download(&self, name: &str, version: &Version) -> Result<Vec<(String, String)>, String>;
}

/// An in-memory fake registry — used for tests and offline development
pub struct FakeRegistry {
    packages: HashMap<String, Vec<PackageMetadata>>,
    sources: HashMap<String, Vec<(String, String)>>, // key: "{name}-{version}"
}

impl FakeRegistry {
    pub fn new() -> Self {
        FakeRegistry {
            packages: HashMap::new(),
            sources: HashMap::new(),
        }
    }

    pub fn publish(
        mut self,
        name: &str,
        version: &str,
        deps: Vec<(&str, &str)>,
        files: Vec<(&str, &str)>,
    ) -> Self {
        let v = Version::parse(version).unwrap();
        let dependencies: HashMap<String, VersionReq> = deps
            .into_iter()
            .map(|(n, r)| (n.to_string(), VersionReq::parse(r).unwrap()))
            .collect();

        let meta = PackageMetadata {
            name: name.to_string(),
            version: v.clone(),
            dependencies,
            download_url: format!("fake://{}-{}", name, v),
        };
        self.packages.entry(name.to_string()).or_default().push(meta);

        let key = format!("{}-{}", name, v);
        let file_pairs: Vec<(String, String)> = files
            .into_iter()
            .map(|(p, c)| (p.to_string(), c.to_string()))
            .collect();
        self.sources.insert(key, file_pairs);

        self
    }
}

impl RegistryClient for FakeRegistry {
    fn list_versions(&self, name: &str) -> Vec<Version> {
        self.packages
            .get(name)
            .map(|v| v.iter().map(|m| m.version.clone()).collect())
            .unwrap_or_default()
    }

    fn get_metadata(&self, name: &str, version: &Version) -> Option<PackageMetadata> {
        self.packages.get(name)?.iter().find(|m| &m.version == version).cloned()
    }

    fn download(&self, name: &str, version: &Version) -> Result<Vec<(String, String)>, String> {
        let key = format!("{}-{}", name, version);
        self.sources
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("no source archive found for {}", key))
    }
}

impl Default for FakeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
