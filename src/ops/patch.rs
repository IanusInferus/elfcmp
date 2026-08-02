use crate::{
    cli::PatchArgs,
    elf,
    manifest::{self, MappingFile},
};
use anyhow::{bail, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
    process::Command,
};
use walkdir::WalkDir;

pub fn run(args: PatchArgs) -> Result<()> {
    if !args.directory.is_dir() {
        bail!(
            "bundle directory does not exist: {}",
            args.directory.display()
        );
    }
    let mapping = args
        .mapping
        .as_deref()
        .map(manifest::read_yaml::<MappingFile>)
        .transpose()?;
    let objects = elf_files(&args.directory)?;
    if objects.is_empty() {
        bail!("no ELF files found in {}", args.directory.display());
    }
    if let Some(mapping) = &mapping {
        // Validate every object before patchelf can mutate the first one.
        for object in &objects {
            let info = elf::inspect(object)?;
            for entry in &mapping.mappings {
                if info
                    .imported
                    .contains(&(entry.from.symbol.clone(), entry.from.version.clone()))
                {
                    if let Some(version) = &entry.to.version {
                        if elf::required_version_index(&info, version).is_none() {
                            bail!(
                                "cannot patch {}: target version {} is not an existing requirement",
                                object.display(),
                                version
                            );
                        }
                    }
                }
            }
        }
    }

    let mut rename_file =
        tempfile::NamedTempFile::new().context("creating temporary symbol map")?;
    let mut renames = BTreeMap::new();
    if let Some(mapping) = &mapping {
        for entry in &mapping.mappings {
            if let Some(existing) =
                renames.insert(entry.from.symbol.clone(), entry.to.symbol.clone())
            {
                if existing != entry.to.symbol {
                    bail!("symbol {} maps to multiple targets", entry.from.symbol);
                }
            }
        }
    }
    for (from, to) in &renames {
        writeln!(rename_file, "{from} {to}")?;
    }
    rename_file.flush()?;

    for object in &objects {
        let info = elf::inspect(object)?;
        if info.is_shared_object {
            eprintln!("[patch] library object: {}", object.display());
        }
        for needed in &info.needed {
            eprintln!("[patch] library {needed}, required by {}", object.display());
        }
        if !renames.is_empty() {
            invoke(
                &args.patchelf,
                &[
                    "--rename-dynamic-symbols",
                    rename_file
                        .path()
                        .to_str()
                        .context("non-UTF-8 temporary path")?,
                    object.to_str().context("non-UTF-8 object path")?,
                ],
            )?;
        }
        if let Some(mapping) = &mapping {
            elf::rewrite_existing_versions(object, &mapping.mappings)?;
        }
        if let Some(mapping) = &mapping {
            let mut added_needed = BTreeSet::new();
            for entry in &mapping.mappings {
                if info.imported.iter().any(|(name, version)| {
                    name == &entry.from.symbol && version == &entry.from.version
                }) && entry.from.library != entry.to.library
                    && !info.needed.contains(&entry.to.library)
                    && added_needed.insert(entry.to.library.clone())
                {
                    eprintln!(
                        "[patch] adding library {} to {}",
                        entry.to.library,
                        object.display()
                    );
                    invoke(
                        &args.patchelf,
                        &[
                            "--add-needed",
                            &entry.to.library,
                            object.to_str().context("non-UTF-8 object path")?,
                        ],
                    )?;
                }
            }
        }
        let rpath = if info.is_shared_object {
            "$ORIGIN"
        } else {
            "$ORIGIN/lib"
        };
        invoke(
            &args.patchelf,
            &[
                "--set-rpath",
                rpath,
                object.to_str().context("non-UTF-8 object path")?,
            ],
        )?;
    }
    println!("patched {} ELF objects", objects.len());
    Ok(())
}

fn elf_files(directory: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut result = Vec::new();
    for entry in WalkDir::new(directory).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let bytes = fs::read(entry.path())?;
            if bytes.starts_with(b"\x7fELF") {
                result.push(entry.into_path());
            }
        }
    }
    Ok(result)
}

fn invoke(program: &Path, arguments: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| {
            format!(
                "running {} (install patchelf or use --patchelf)",
                program.display()
            )
        })?;
    if !status.success() {
        bail!("{} failed with {status}", program.display());
    }
    Ok(())
}
