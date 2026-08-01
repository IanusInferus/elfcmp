use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs::File, path::Path};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct SymbolRef {
    pub library: String,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReferenceTable {
    pub format: u32,
    #[serde(alias = "executable")]
    pub input: String,
    pub symbols: BTreeSet<SymbolRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<ObjectReference>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ImportedSymbol {
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct VersionRequirement {
    pub library: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectReference {
    pub object: String,
    pub imports: BTreeSet<ImportedSymbol>,
    pub required_versions: BTreeSet<VersionRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEndpoint {
    pub library: String,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingEntry {
    pub from: SymbolEndpoint,
    pub to: SymbolEndpoint,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MappingFile {
    pub format: u32,
    pub mappings: Vec<MappingEntry>,
}

pub fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    serde_yaml::from_reader(file).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_yaml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    serde_yaml::to_writer(file, value).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_round_trips_with_versions() {
        let input = MappingFile {
            format: 1,
            mappings: vec![MappingEntry {
                from: SymbolEndpoint {
                    library: "libc.so.6".into(),
                    symbol: "old".into(),
                    version: Some("GLIBC_2.17".into()),
                },
                to: SymbolEndpoint {
                    library: "libcompat.so.1".into(),
                    symbol: "new".into(),
                    version: None,
                },
            }],
        };
        let yaml = serde_yaml::to_string(&input).unwrap();
        let output: MappingFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(output.format, 1);
        assert_eq!(
            output.mappings[0].from.version.as_deref(),
            Some("GLIBC_2.17")
        );
        assert_eq!(output.mappings[0].to.library, "libcompat.so.1");
    }

    #[test]
    fn absent_version_is_omitted() {
        let endpoint = SymbolEndpoint {
            library: "libx.so".into(),
            symbol: "x".into(),
            version: None,
        };
        let yaml = serde_yaml::to_string(&endpoint).unwrap();
        assert!(!yaml.contains("version"));
    }
}
