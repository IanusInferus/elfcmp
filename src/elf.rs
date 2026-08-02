use crate::manifest::ElfArchitecture;
use anyhow::{bail, Context, Result};
use goblin::elf::{
    section_header::{SHT_GNU_VERNEED, SHT_GNU_VERSYM},
    sym, Elf,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct ElfInfo {
    pub architecture: ElfArchitecture,
    pub is_shared_object: bool,
    pub needed: Vec<String>,
    pub imported: BTreeSet<(String, Option<String>)>,
    pub exported: BTreeSet<(String, Option<String>)>,
    pub required_versions: BTreeMap<(String, String), u16>,
    pub runpaths: Vec<String>,
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
        architecture: ElfArchitecture {
            machine: elf.header.e_machine,
            bits: if elf.is_64 { 64 } else { 32 },
            endianness: if elf.little_endian { "little" } else { "big" }.to_owned(),
        },
        is_shared_object: elf.header.e_type == goblin::elf::header::ET_DYN
            && elf.interpreter.is_none(),
        needed: elf.libraries.iter().map(|s| (*s).to_owned()).collect(),
        imported,
        exported,
        required_versions,
        runpaths: elf
            .runpaths
            .iter()
            .flat_map(|path| path.split(':'))
            .map(str::to_owned)
            .collect(),
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
    let mut changes = BTreeMap::new();

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
                        need.iter().find_map(|aux| {
                            (elf.dynstrtab.get_at(aux.vna_name) == Some(version.as_str()))
                                .then_some(aux.vna_other & 0x7fff)
                        })
                    })
                })
                .with_context(|| {
                    format!(
                        "target version {} is not already required by {}",
                        version,
                        path.display()
                    )
                })?,
        };
        for symbol_index in matching_symbols {
            changes.insert(symbol_index, target_index);
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
    let mut used_versions = BTreeSet::new();
    for (index, symbol) in elf.dynsyms.iter().enumerate() {
        if symbol.st_shndx != 0 {
            continue;
        }
        let version_index = changes
            .get(&index)
            .copied()
            .or_else(|| {
                elf.versym
                    .as_ref()?
                    .get_at(index)
                    .map(|version| version.version())
            })
            .unwrap_or(1)
            & 0x7fff;
        if version_index > 1 {
            used_versions.insert(version_index);
        }
    }
    let (cleanup_edits, removed_requirements) = verneed_cleanup_edits(&data, &elf, &used_versions)?;
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
    for (offset, bytes) in cleanup_edits {
        data.get_mut(offset..offset + bytes.len())
            .context(".gnu.version_r edit lies outside the ELF file")?
            .copy_from_slice(&bytes);
    }
    fs::write(path, data).with_context(|| format!("writing ELF {}", path.display()))?;
    if removed_requirements > 0 {
        eprintln!(
            "[patch] removed {removed_requirements} unused version requirements from {}",
            path.display()
        );
    }
    Ok(())
}

fn verneed_cleanup_edits(
    data: &[u8],
    elf: &Elf<'_>,
    used_versions: &BTreeSet<u16>,
) -> Result<(Vec<(usize, Vec<u8>)>, usize)> {
    let Some(section) = elf
        .section_headers
        .iter()
        .find(|section| section.sh_type == SHT_GNU_VERNEED)
    else {
        return Ok((Vec::new(), 0));
    };
    let start = section.sh_offset as usize;
    let end = start
        .checked_add(section.sh_size as usize)
        .filter(|end| *end <= data.len())
        .context(".gnu.version_r lies outside the ELF file")?;
    let mut edits = Vec::new();
    let mut removed = 0;
    let mut need_offset = start;
    loop {
        require_range(need_offset, 16, end, ".gnu.version_r Verneed")?;
        let count = read_u16(data, need_offset + 2, elf.little_endian)? as usize;
        let aux_relative = read_u32(data, need_offset + 8, elf.little_endian)? as usize;
        let next_need = read_u32(data, need_offset + 12, elf.little_endian)? as usize;
        let mut auxiliaries = Vec::new();
        if count > 0 {
            let mut aux_offset = need_offset
                .checked_add(aux_relative)
                .context(".gnu.version_r auxiliary offset overflow")?;
            for index in 0..count {
                require_range(aux_offset, 16, end, ".gnu.version_r Vernaux")?;
                let version = read_u16(data, aux_offset + 6, elf.little_endian)? & 0x7fff;
                auxiliaries.push((aux_offset, version));
                let next_aux = read_u32(data, aux_offset + 12, elf.little_endian)? as usize;
                if index + 1 < count {
                    if next_aux == 0 {
                        bail!("truncated .gnu.version_r auxiliary chain");
                    }
                    aux_offset = aux_offset
                        .checked_add(next_aux)
                        .context(".gnu.version_r auxiliary offset overflow")?;
                }
            }
        }
        let kept: Vec<_> = auxiliaries
            .iter()
            .filter(|(_, version)| used_versions.contains(version))
            .map(|(offset, _)| *offset)
            .collect();
        removed += auxiliaries.len() - kept.len();
        edits.push((
            need_offset + 2,
            encode_u16(kept.len() as u16, elf.little_endian).to_vec(),
        ));
        edits.push((
            need_offset + 8,
            encode_u32(
                kept.first().map_or(0, |offset| offset - need_offset) as u32,
                elf.little_endian,
            )
            .to_vec(),
        ));
        for (index, offset) in kept.iter().enumerate() {
            let next = kept.get(index + 1).map_or(0, |next| next - offset) as u32;
            edits.push((offset + 12, encode_u32(next, elf.little_endian).to_vec()));
        }
        if next_need == 0 {
            break;
        }
        need_offset = need_offset
            .checked_add(next_need)
            .filter(|offset| *offset < end)
            .context("invalid .gnu.version_r Verneed chain")?;
    }
    Ok((edits, removed))
}

fn require_range(offset: usize, size: usize, end: usize, description: &str) -> Result<()> {
    if offset.checked_add(size).is_none_or(|limit| limit > end) {
        bail!("{description} lies outside its section");
    }
    Ok(())
}

fn read_u16(data: &[u8], offset: usize, little_endian: bool) -> Result<u16> {
    let bytes: [u8; 2] = data
        .get(offset..offset + 2)
        .context("reading 16-bit ELF field")?
        .try_into()?;
    Ok(if little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    })
}

fn read_u32(data: &[u8], offset: usize, little_endian: bool) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .context("reading 32-bit ELF field")?
        .try_into()?;
    Ok(if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    })
}

fn encode_u16(value: u16, little_endian: bool) -> [u8; 2] {
    if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    }
}

fn encode_u32(value: u32, little_endian: bool) -> [u8; 4] {
    if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    }
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

pub fn is_shared_library_filename(name: &str) -> bool {
    if !name.starts_with("lib") {
        return false;
    }
    let components: Vec<_> = name.split('.').collect();
    let mut suffix_start = components.len();
    while suffix_start > 0
        && !components[suffix_start - 1].is_empty()
        && components[suffix_start - 1]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        suffix_start -= 1;
    }
    suffix_start > 1 && components[suffix_start - 1] == "so"
}

pub fn exports_symbol(info: &ElfInfo, symbol: &str, required_version: Option<&str>) -> bool {
    info.exported.iter().any(|(exported, version)| {
        exported == symbol
            && required_version.is_none_or(|required| version.as_deref() == Some(required))
    })
}

pub fn required_version_index(info: &ElfInfo, version: &str) -> Option<u16> {
    info.required_versions
        .iter()
        .find_map(|((_, required), index)| (required == version).then_some(*index))
}

pub fn find_library(
    sysroot: &Path,
    name: &str,
    extra: &[PathBuf],
    expected_architecture: Option<&ElfArchitecture>,
) -> Result<PathBuf> {
    let mut incompatible = Vec::new();
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
            if library_architecture_matches(&candidate, expected_architecture) {
                return Ok(candidate);
            }
            incompatible.push(candidate);
        }
    }
    // Multiarch directories are common and cheap to search one level deep.
    for base in [sysroot.join("lib"), sysroot.join("usr/lib")] {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten().filter(|e| e.path().is_dir()) {
                let candidate = entry.path().join(name);
                if candidate.is_file() {
                    if library_architecture_matches(&candidate, expected_architecture) {
                        return Ok(candidate);
                    }
                    incompatible.push(candidate);
                }
            }
        }
    }
    if incompatible.is_empty() {
        bail!(
            "library {name} not found beneath sysroot {}",
            sysroot.display()
        )
    } else {
        bail!(
            "library {name} has no matching architecture beneath sysroot {}; rejected: {}",
            sysroot.display(),
            incompatible
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

pub fn find_host_library(
    name: &str,
    directories: &[PathBuf],
    expected_architecture: Option<&ElfArchitecture>,
) -> Result<PathBuf> {
    let mut incompatible = Vec::new();
    for directory in directories {
        let candidate = directory.join(name);
        if candidate.is_file() {
            if library_architecture_matches(&candidate, expected_architecture) {
                return Ok(candidate);
            }
            incompatible.push(candidate);
        }
    }
    if incompatible.is_empty() {
        bail!("host library {name} not found in the configured paths")
    }
    bail!(
        "host library {name} has no matching architecture; rejected: {}",
        incompatible
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn library_architecture_matches(
    path: &Path,
    expected: Option<&ElfArchitecture>,
) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    inspect(path)
        .map(|info| architectures_match(expected, &info.architecture))
        .unwrap_or(false)
}

fn architectures_match(expected: &ElfArchitecture, actual: &ElfArchitecture) -> bool {
    expected == actual
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_library_filename_supports_dotted_names_and_versions() {
        assert!(is_shared_library_filename("liba.b.c.so.1.2.3"));
        assert!(is_shared_library_filename("liba.so"));
        assert!(is_shared_library_filename("liba.so.1"));
        assert!(!is_shared_library_filename("a.b.c.so.1.2.3"));
        assert!(!is_shared_library_filename("liba.so.debug"));
        assert!(!is_shared_library_filename("liba.so.1.debug"));
        assert!(!is_shared_library_filename("libapplication"));
    }

    #[test]
    fn architecture_requires_machine_bitness_and_endianness() {
        let x86_64 = ElfArchitecture {
            machine: goblin::elf::header::EM_X86_64,
            bits: 64,
            endianness: "little".into(),
        };
        assert!(architectures_match(&x86_64, &x86_64));
        assert!(!architectures_match(
            &x86_64,
            &ElfArchitecture {
                machine: goblin::elf::header::EM_386,
                bits: 32,
                endianness: "little".into(),
            }
        ));
        assert!(!architectures_match(
            &x86_64,
            &ElfArchitecture {
                endianness: "big".into(),
                ..x86_64
            }
        ));
    }

    #[test]
    fn unversioned_requirement_matches_versioned_export() {
        let mut exported = BTreeSet::new();
        exported.insert(("function".to_owned(), Some("VERSION_1".to_owned())));
        let info = ElfInfo {
            architecture: ElfArchitecture {
                machine: goblin::elf::header::EM_X86_64,
                bits: 64,
                endianness: "little".into(),
            },
            is_shared_object: true,
            needed: Vec::new(),
            imported: BTreeSet::new(),
            exported,
            required_versions: BTreeMap::new(),
            runpaths: Vec::new(),
        };
        assert!(exports_symbol(&info, "function", None));
        assert!(exports_symbol(&info, "function", Some("VERSION_1")));
        assert!(!exports_symbol(&info, "function", Some("VERSION_2")));
        assert!(!exports_symbol(&info, "other", None));
    }

    #[test]
    fn version_index_can_be_reused_from_another_library() {
        let mut required_versions = BTreeMap::new();
        required_versions.insert(("libc.so.6".to_owned(), "GLIBC_2.2.5".to_owned()), 3);
        let info = ElfInfo {
            architecture: ElfArchitecture {
                machine: goblin::elf::header::EM_X86_64,
                bits: 64,
                endianness: "little".into(),
            },
            is_shared_object: false,
            needed: Vec::new(),
            imported: BTreeSet::new(),
            exported: BTreeSet::new(),
            required_versions,
            runpaths: Vec::new(),
        };
        assert_eq!(required_version_index(&info, "GLIBC_2.2.5"), Some(3));
        assert_eq!(required_version_index(&info, "GLIBC_2.34"), None);
    }
}
