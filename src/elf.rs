use anyhow::{bail, Context, Result};
use goblin::elf::{section_header::SHT_GNU_VERSYM, sym, Elf};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct ElfInfo {
    pub needed: Vec<String>,
    pub imported: BTreeSet<(String, Option<String>)>,
    pub exported: BTreeSet<(String, Option<String>)>,
    pub required_versions: BTreeMap<(String, String), u16>,
}

pub fn inspect(path: &Path) -> Result<ElfInfo> {
    let data = fs::read(path).with_context(|| format!("reading ELF {}", path.display()))?;
    let elf = Elf::parse(&data).with_context(|| format!("parsing ELF {}", path.display()))?;
    let mut imported = BTreeSet::new();
    let mut exported = BTreeSet::new();
    for (index, symbol) in elf.dynsyms.iter().enumerate() {
        if symbol.st_name == 0
            || symbol.st_type() == sym::STT_FILE
            || symbol.st_type() == sym::STT_SECTION
        {
            continue;
        }
        let Some(name) = elf.dynstrtab.get_at(symbol.st_name) else {
            continue;
        };
        let version = symbol_version(&elf, index);
        if symbol.st_shndx == 0 {
            imported.insert((name.to_owned(), version));
        } else if symbol.st_bind() == sym::STB_GLOBAL || symbol.st_bind() == sym::STB_WEAK {
            exported.insert((name.to_owned(), version));
        }
    }
    let mut required_versions = BTreeMap::new();
    if let Some(verneed) = elf.verneed.as_ref() {
        for need in verneed.iter() {
            let Some(library) = elf.dynstrtab.get_at(need.vn_file) else {
                continue;
            };
            for aux in need.iter() {
                if let Some(version) = elf.dynstrtab.get_at(aux.vna_name) {
                    required_versions.insert(
                        (library.to_owned(), version.to_owned()),
                        aux.vna_other & 0x7fff,
                    );
                }
            }
        }
    }
    Ok(ElfInfo {
        needed: elf.libraries.iter().map(|s| (*s).to_owned()).collect(),
        imported,
        exported,
        required_versions,
    })
}

/// Repoint imported symbols to version requirements that already exist in this ELF.
/// No section is resized.
pub fn rewrite_existing_versions(
    path: &Path,
    mappings: &[crate::manifest::MappingEntry],
) -> Result<()> {
    if !mappings.iter().any(|mapping| {
        mapping.from.version != mapping.to.version || mapping.from.library != mapping.to.library
    }) {
        return Ok(());
    }
    let mut data = fs::read(path).with_context(|| format!("reading ELF {}", path.display()))?;
    let elf = Elf::parse(&data).with_context(|| format!("parsing ELF {}", path.display()))?;
    let little_endian = elf.little_endian;
    let mut changes = Vec::new();

    for mapping in mappings {
        if mapping.from.version == mapping.to.version && mapping.from.library == mapping.to.library
        {
            continue;
        }
        let matching_symbols: Vec<_> = elf
            .dynsyms
            .iter()
            .enumerate()
            .filter(|(index, symbol)| {
                symbol.st_shndx == 0
                    && elf.dynstrtab.get_at(symbol.st_name) == Some(mapping.to.symbol.as_str())
                    && symbol_version(&elf, *index) == mapping.from.version
            })
            .map(|(index, _)| index)
            .collect();
        if matching_symbols.is_empty() {
            continue;
        }
        let target_index = match &mapping.to.version {
            None => 1,
            Some(version) => elf
                .verneed
                .as_ref()
                .and_then(|section| {
                    section.iter().find_map(|need| {
                        (elf.dynstrtab.get_at(need.vn_file) == Some(mapping.to.library.as_str()))
                            .then(|| {
                                need.iter().find_map(|aux| {
                                    (elf.dynstrtab.get_at(aux.vna_name) == Some(version.as_str()))
                                        .then_some(aux.vna_other & 0x7fff)
                                })
                            })
                            .flatten()
                    })
                })
                .with_context(|| {
                    format!(
                        "target version {} from {} is not already required by {}",
                        version,
                        mapping.to.library,
                        path.display()
                    )
                })?,
        };
        for symbol_index in matching_symbols {
            changes.push((symbol_index, target_index));
        }
    }
    if changes.is_empty() {
        return Ok(());
    }
    let versym_offset = elf
        .section_headers
        .iter()
        .find(|section| section.sh_type == SHT_GNU_VERSYM)
        .map(|section| section.sh_offset as usize)
        .context("ELF has versioned imports but no .gnu.version section")?;
    drop(elf);
    for (symbol_index, version_index) in changes {
        let offset = versym_offset + symbol_index * 2;
        let destination = data
            .get_mut(offset..offset + 2)
            .context(".gnu.version entry lies outside the ELF file")?;
        let encoded = if little_endian {
            version_index.to_le_bytes()
        } else {
            version_index.to_be_bytes()
        };
        destination.copy_from_slice(&encoded);
    }
    fs::write(path, data).with_context(|| format!("writing ELF {}", path.display()))
}

fn symbol_version(elf: &Elf<'_>, index: usize) -> Option<String> {
    let versym = elf.versym.as_ref()?.get_at(index)?;
    let version_index = versym.version();
    if version_index <= 1 {
        return None;
    }
    if let Some(verneed) = elf.verneed.as_ref() {
        for need in verneed.iter() {
            for aux in need.iter() {
                if aux.vna_other == version_index {
                    return elf.dynstrtab.get_at(aux.vna_name).map(str::to_owned);
                }
            }
        }
    }
    if let Some(verdef) = elf.verdef.as_ref() {
        for definition in verdef.iter() {
            if definition.vd_ndx == version_index {
                if let Some(aux) = definition.iter().next() {
                    return elf.dynstrtab.get_at(aux.vda_name).map(str::to_owned);
                }
            }
        }
    }
    None
}

pub fn default_search_paths() -> Vec<PathBuf> {
    ["lib", "lib64", "usr/lib", "usr/lib64", "usr/local/lib"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

pub fn find_library(sysroot: &Path, name: &str, extra: &[PathBuf]) -> Result<PathBuf> {
    let paths = extra.iter().cloned().chain(default_search_paths());
    for directory in paths {
        let relative = if directory.is_absolute() {
            directory
                .strip_prefix(directory.ancestors().last().unwrap())
                .unwrap_or(&directory)
        } else {
            directory.as_path()
        };
        let candidate = sysroot.join(relative).join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    // Multiarch directories are common and cheap to search one level deep.
    for base in [sysroot.join("lib"), sysroot.join("usr/lib")] {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten().filter(|e| e.path().is_dir()) {
                let candidate = entry.path().join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }
    bail!(
        "library {name} not found beneath sysroot {}",
        sysroot.display()
    )
}
