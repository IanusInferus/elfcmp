use crate::{
    cli::MapArgs,
    elf,
    manifest::{self, MappingEntry, MappingFile, ReferenceTable, SymbolEndpoint},
};
use anyhow::Result;
use std::collections::BTreeMap;

pub fn run(args: MapArgs) -> Result<()> {
    let reference: ReferenceTable = manifest::read_yaml(&args.reference)?;
    let mut mappings = Vec::new();
    let mut libraries = BTreeMap::new();
    for required in &reference.symbols {
        if !libraries.contains_key(&required.library) {
            let library = match elf::find_library(
                &args.target_sysroot,
                &required.library,
                &args.system_lib_search_paths,
                reference.architecture.as_ref(),
            ) {
                Ok(path) => {
                    eprintln!("[map] library {}: {}", required.library, path.display());
                    elf::inspect(&path).ok()
                }
                Err(_) => None,
            };
            libraries.insert(required.library.clone(), library);
        }
        let present = libraries
            .get(&required.library)
            .and_then(Option::as_ref)
            .is_some_and(|info| {
                elf::exports_symbol(info, &required.symbol, required.version.as_deref())
            });
        if !present {
            let endpoint = SymbolEndpoint {
                library: required.library.clone(),
                symbol: required.symbol.clone(),
                version: required.version.clone(),
            };
            mappings.push(MappingEntry {
                from: endpoint.clone(),
                to: endpoint,
            });
        }
    }
    let missing = mappings.len();
    manifest::write_yaml(
        &args.output,
        &MappingFile {
            format: 1,
            mappings,
        },
    )?;
    println!(
        "wrote {missing} missing-symbol mappings to {}",
        args.output.display()
    );
    Ok(())
}
