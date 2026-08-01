use crate::{
    cli::MapArgs,
    elf,
    manifest::{self, MappingEntry, MappingFile, ReferenceTable, SymbolEndpoint},
};
use anyhow::Result;

pub fn run(args: MapArgs) -> Result<()> {
    let reference: ReferenceTable = manifest::read_yaml(&args.reference)?;
    let mut mappings = Vec::new();
    for required in &reference.symbols {
        let present = elf::find_library(
            &args.target_sysroot,
            &required.library,
            &args.system_lib_search_paths,
        )
        .and_then(|path| elf::inspect(&path))
        .map(|info| {
            info.exported
                .contains(&(required.symbol.clone(), required.version.clone()))
        })
        .unwrap_or(false);
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
