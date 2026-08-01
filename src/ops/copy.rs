use crate::{
    cli::CopyArgs,
    elf,
    manifest::{
        self, ImportedSymbol, ObjectReference, ReferenceTable, SymbolRef, VersionRequirement,
    },
};
use anyhow::{bail, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

const SYSTEM_LIB_BASENAMES: &[&str] =
    &["libc", "libdl", "libm", "libpthread", "librt", "libselinux"];

pub fn run(args: CopyArgs) -> Result<()> {
    if !args.input.is_file() {
        bail!("input does not exist: {}", args.input.display());
    }
    let input_architecture = elf::inspect(&args.input)?.architecture;
    let input_name = args
        .input
        .file_name()
        .context("input path has no file name")?;
    let input_is_library = elf::is_shared_library_filename(&input_name.to_string_lossy());
    if input_is_library {
        eprintln!("[copy] library input: {}", args.input.display());
    }
    let dependency_directory = if input_is_library {
        args.output.clone()
    } else {
        args.output.join("lib")
    };
    fs::create_dir_all(&dependency_directory)
        .with_context(|| format!("creating {}", dependency_directory.display()))?;
    copy_file(&args.input, &args.output.join(input_name))?;

    let basenames = system_lib_basenames(args.system_lib_basenames);
    let mut queue = VecDeque::from([args.input.clone()]);
    let mut visited = BTreeSet::new();
    let mut system_objects = BTreeMap::new();
    let mut consumers = Vec::new();

    while let Some(object) = queue.pop_front() {
        let info = elf::inspect(&object)?;
        let object_name = if object == args.input {
            input_name.to_string_lossy().into_owned()
        } else {
            let dependency_name = object
                .file_name()
                .context("dependency path has no file name")?
                .to_string_lossy();
            if input_is_library {
                dependency_name.into_owned()
            } else {
                format!("lib/{dependency_name}")
            }
        };
        consumers.push(ObjectReference {
            object: object_name,
            imports: info
                .imported
                .iter()
                .map(|(symbol, version)| ImportedSymbol {
                    symbol: symbol.clone(),
                    version: version.clone(),
                })
                .collect(),
            required_versions: info
                .required_versions
                .keys()
                .map(|(library, version)| VersionRequirement {
                    library: library.clone(),
                    version: version.clone(),
                })
                .collect(),
        });
        for name in info.needed {
            if !visited.insert(name.clone()) {
                continue;
            }
            let source = elf::find_library(
                &args.sysroot,
                &name,
                &args.system_lib_search_paths,
                Some(&info.architecture),
            )?;
            eprintln!("[copy] library {name}: {}", source.display());
            let dependency = elf::inspect(&source)?;
            if is_system_library(&name, &basenames) {
                system_objects.insert(name, dependency);
            } else {
                let destination = dependency_directory.join(&name);
                copy_file(&source, &destination)?;
                queue.push_back(source);
            }
        }
    }

    let mut symbols = BTreeSet::new();
    for consumer in &consumers {
        for import in &consumer.imports {
            let symbol = &import.symbol;
            let version = &import.version;
            for (library, provider) in &system_objects {
                if provider
                    .exported
                    .contains(&(symbol.clone(), version.clone()))
                    || (version.is_none()
                        && provider.exported.iter().any(|(name, _)| name == symbol))
                {
                    symbols.insert(SymbolRef {
                        library: library.clone(),
                        symbol: symbol.to_owned(),
                        version: version.clone(),
                    });
                    break;
                }
            }
        }
    }
    let reference_path = args
        .reference
        .unwrap_or_else(|| args.output.join("elfcmp-reference.yaml"));
    manifest::write_yaml(
        &reference_path,
        &ReferenceTable {
            format: 1,
            input: input_name.to_string_lossy().into_owned(),
            architecture: Some(input_architecture),
            symbols,
            objects: consumers,
        },
    )?;
    println!(
        "copied {} ELF objects; wrote {}",
        visited.len() + 1 - system_objects.len(),
        reference_path.display()
    );
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    if fs::copy(source, destination).is_ok() {
        return Ok(());
    }
    let mut input = File::open(source)
        .with_context(|| format!("opening {} for buffered copy", source.display()))?;
    let mut output = File::create(destination)
        .with_context(|| format!("creating {} for buffered copy", destination.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .with_context(|| format!("reading {} during buffered copy", source.display()))?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count]).with_context(|| {
            format!(
                "writing {} during buffered copy from {}",
                destination.display(),
                source.display()
            )
        })?;
    }
    let permissions = fs::metadata(source)
        .with_context(|| format!("reading permissions for {}", source.display()))?
        .permissions();
    fs::set_permissions(destination, permissions)
        .with_context(|| format!("setting permissions on {}", destination.display()))
}

fn system_lib_basenames(specified: Vec<String>) -> Vec<String> {
    if specified.is_empty() {
        SYSTEM_LIB_BASENAMES
            .iter()
            .map(|prefix| (*prefix).to_owned())
            .collect()
    } else {
        specified
    }
}

fn is_system_library(soname: &str, base_names: &[String]) -> bool {
    let base_name = soname_basename(soname);
    base_names.iter().any(|candidate| candidate == &base_name)
}

fn soname_basename(soname: &str) -> String {
    let components: Vec<_> = soname.split('.').collect();
    let mut suffix_start = components.len();
    while suffix_start > 0
        && !components[suffix_start - 1].is_empty()
        && components[suffix_start - 1]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        suffix_start -= 1;
    }
    if suffix_start > 0 && components[suffix_start - 1] == "so" {
        components[..suffix_start - 1].join(".")
    } else {
        soname.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_basenames_use_defaults() {
        assert_eq!(
            system_lib_basenames(Vec::new()),
            ["libc", "libdl", "libm", "libpthread", "librt", "libselinux"]
        );
    }

    #[test]
    fn specified_basenames_replace_defaults() {
        assert_eq!(
            system_lib_basenames(vec!["libplatform".into(), "libvendor".into()]),
            ["libplatform", "libvendor"]
        );
    }

    #[test]
    fn base_names_match_versioned_sonames() {
        let base_names = system_lib_basenames(vec!["libplatform".into(), "libvendor".into()]);
        assert!(is_system_library("libplatform.so.1", &base_names));
        assert!(is_system_library("libvendor.so.6", &base_names));
    }

    #[test]
    fn base_name_matching_does_not_cross_library_boundaries() {
        let base_names = system_lib_basenames(Vec::new());
        assert!(is_system_library("libc.so.6", &base_names));
        assert!(!is_system_library("libc++.so.1", &base_names));
        assert!(!is_system_library("libcrypt.so.1", &base_names));
        assert!(is_system_library("libm.so.6", &base_names));
        assert!(!is_system_library("libmount.so.1", &base_names));
    }

    #[test]
    fn dotted_basename_preserves_name_components() {
        let base_names = vec!["liba.b.c".to_owned()];
        assert!(is_system_library("liba.b.c.so.1.2.3", &base_names));
        assert!(!is_system_library("liba.so.1.2.3", &base_names));
        assert!(!is_system_library("liba.b.c.extra.so.1", &base_names));
    }

    #[test]
    fn only_final_so_and_numeric_suffix_are_removed() {
        assert_eq!(soname_basename("liba.b.c.so.1.2.3"), "liba.b.c");
        assert_eq!(soname_basename("liba.so"), "liba");
        assert_eq!(soname_basename("liba.so.debug.1"), "liba.so.debug.1");
        assert_eq!(soname_basename("liba.1"), "liba.1");
    }
}
