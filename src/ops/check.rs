use crate::{
    cli::CheckArgs,
    elf,
    manifest::{self, MappingFile, ReferenceTable, SymbolEndpoint, SymbolRef},
};
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

fn endpoint_exists(
    root: &std::path::Path,
    endpoint: &SymbolEndpoint,
    paths: &[std::path::PathBuf],
    architecture: Option<&crate::manifest::ElfArchitecture>,
    libraries: &mut BTreeMap<String, Option<elf::ElfInfo>>,
) -> bool {
    if !libraries.contains_key(&endpoint.library) {
        let library = match elf::find_library(root, &endpoint.library, paths, architecture) {
            Ok(path) => {
                eprintln!("[check] library {}: {}", endpoint.library, path.display());
                elf::inspect(&path).ok()
            }
            Err(_) => None,
        };
        libraries.insert(endpoint.library.clone(), library);
    }
    libraries
        .get(&endpoint.library)
        .and_then(Option::as_ref)
        .is_some_and(|info| {
            elf::exports_symbol(info, &endpoint.symbol, endpoint.version.as_deref())
        })
}

pub fn run(args: CheckArgs) -> Result<()> {
    let reference: ReferenceTable = manifest::read_yaml(&args.reference)?;
    let mapping: MappingFile = manifest::read_yaml(&args.mapping)?;
    let mut errors = Vec::new();
    let referenced: BTreeSet<_> = reference.symbols.iter().cloned().collect();
    let mut mapped_sources = BTreeSet::new();
    let mut libraries = BTreeMap::new();
    let mut missing_targets = 0_usize;
    for (index, entry) in mapping.mappings.iter().enumerate() {
        let source = SymbolRef {
            library: entry.from.library.clone(),
            symbol: entry.from.symbol.clone(),
            version: entry.from.version.clone(),
        };
        if !referenced.contains(&source) {
            errors.push(format!(
                "mapping #{index}: source is not present in {}",
                args.reference.display()
            ));
        }
        if !mapped_sources.insert(source) {
            errors.push(format!("mapping #{index}: duplicate source mapping"));
        }
        if !endpoint_exists(
            &args.target_sysroot,
            &entry.to,
            &args.system_lib_search_paths,
            reference.architecture.as_ref(),
            &mut libraries,
        ) {
            missing_targets += 1;
            eprintln!(
                "[check] unresolved function: from library={} symbol={} version={} -> to library={} symbol={} version={}",
                entry.from.library,
                entry.from.symbol,
                entry.from.version.as_deref().unwrap_or("<unversioned>"),
                entry.to.library,
                entry.to.symbol,
                entry.to.version.as_deref().unwrap_or("<unversioned>")
            );
        }
    }
    if missing_targets > 0 {
        errors.push(format!(
            "{missing_targets} mapping target symbols are not exported; see the unresolved-function logs above"
        ));
    }
    for required in &reference.symbols {
        let original = SymbolEndpoint {
            library: required.library.clone(),
            symbol: required.symbol.clone(),
            version: required.version.clone(),
        };
        let missing = !endpoint_exists(
            &args.target_sysroot,
            &original,
            &args.system_lib_search_paths,
            reference.architecture.as_ref(),
            &mut libraries,
        );
        if missing && !mapped_sources.contains(required) {
            errors.push(format!(
                "missing source mapping for {}/{}{}",
                required.library,
                required.symbol,
                required
                    .version
                    .as_ref()
                    .map(|version| format!("@{version}"))
                    .unwrap_or_default()
            ));
        }
    }
    for object in &reference.objects {
        for (index, entry) in mapping.mappings.iter().enumerate() {
            let applies = object.imports.iter().any(|import| {
                import.symbol == entry.from.symbol && import.version == entry.from.version
            });
            if applies {
                if let Some(version) = &entry.to.version {
                    if !object.required_versions.iter().any(|requirement| {
                        requirement.library == entry.to.library && requirement.version == *version
                    }) {
                        errors.push(format!(
                            "mapping #{index}: {} does not already require {}/{}",
                            object.object, entry.to.library, version
                        ));
                    }
                }
            }
        }
    }
    if !errors.is_empty() {
        bail!("mapping is invalid:\n  - {}", errors.join("\n  - "));
    }
    println!("mapping is valid ({} entries)", mapping.mappings.len());
    Ok(())
}
