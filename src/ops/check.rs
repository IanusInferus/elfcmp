use crate::{
    cli::CheckArgs,
    elf,
    manifest::{self, MappingFile, ReferenceTable, SymbolEndpoint, SymbolRef},
};
use anyhow::{bail, Result};
use std::collections::BTreeSet;

fn endpoint_exists(
    root: &std::path::Path,
    endpoint: &SymbolEndpoint,
    paths: &[std::path::PathBuf],
) -> Result<bool> {
    let object = elf::find_library(root, &endpoint.library, paths)?;
    let info = elf::inspect(&object)?;
    Ok(info
        .exported
        .contains(&(endpoint.symbol.clone(), endpoint.version.clone()))
        || endpoint.version.is_none()
            && info
                .exported
                .iter()
                .any(|(name, _)| name == &endpoint.symbol))
}

pub fn run(args: CheckArgs) -> Result<()> {
    let reference: ReferenceTable = manifest::read_yaml(&args.reference)?;
    let mapping: MappingFile = manifest::read_yaml(&args.mapping)?;
    let mut errors = Vec::new();
    let referenced: BTreeSet<_> = reference.symbols.iter().cloned().collect();
    let mut mapped_sources = BTreeSet::new();
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
        match endpoint_exists(
            &args.target_sysroot,
            &entry.to,
            &args.system_lib_search_paths,
        ) {
            Ok(true) => {}
            Ok(false) => errors.push(format!("mapping #{index}: target symbol is not exported")),
            Err(e) => errors.push(format!("mapping #{index}: target: {e:#}")),
        }
    }
    for required in &reference.symbols {
        let original = SymbolEndpoint {
            library: required.library.clone(),
            symbol: required.symbol.clone(),
            version: required.version.clone(),
        };
        let missing = endpoint_exists(
            &args.target_sysroot,
            &original,
            &args.system_lib_search_paths,
        )
        .map(|exists| !exists)
        .unwrap_or(true);
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
