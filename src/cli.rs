use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "elfcmp", version, about = "ELF copy-map-patch toolkit")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Copy an executable and its non-system shared-library closure.
    Copy(CopyArgs),
    /// Compare a reference ABI against a target sysroot and emit a mapping template.
    Map(MapArgs),
    /// Validate a completed mapping against a reference table and target sysroot.
    Check(CheckArgs),
    /// Apply a mapping and make a copied tree relocatable with $ORIGIN RPATHs.
    Patch(PatchArgs),
}

#[derive(Debug, Args)]
pub struct CopyArgs {
    /// ELF executable to bundle.
    pub executable: PathBuf,
    /// Destination directory (the executable is placed at its root and DSOs in lib/).
    pub output: PathBuf,
    /// Source sysroot. Defaults to /.
    #[arg(long, default_value = "/")]
    pub sysroot: PathBuf,
    /// Additional colon-separated library directories, interpreted inside the sysroot.
    #[arg(long, value_delimiter = ':')]
    pub system_lib_search_paths: Vec<PathBuf>,
    /// SONAME base names treated as system libraries. Replaces the defaults when specified.
    #[arg(long, value_delimiter = ',')]
    pub system_lib_basenames: Vec<String>,
    /// Reference-table output. Defaults to OUTPUT/elfcmp-reference.yaml.
    #[arg(long)]
    pub reference: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MapArgs {
    /// Reference table produced by `elfcmp copy`.
    pub reference: PathBuf,
    /// Target sysroot to inspect.
    pub target_sysroot: PathBuf,
    /// Mapping-template output.
    pub output: PathBuf,
    /// Additional colon-separated library directories inside the target sysroot.
    #[arg(long, value_delimiter = ':')]
    pub system_lib_search_paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Reference table produced by `elfcmp copy`.
    pub reference: PathBuf,
    /// Target sysroot to inspect.
    pub target_sysroot: PathBuf,
    /// Completed mapping file.
    pub mapping: PathBuf,
    /// Additional colon-separated library directories inside the target sysroot.
    #[arg(long, value_delimiter = ':')]
    pub system_lib_search_paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PatchArgs {
    /// Bundle directory produced by `elfcmp copy`.
    pub directory: PathBuf,
    /// Optional completed mapping file.
    #[arg(short, long)]
    pub mapping: Option<PathBuf>,
    /// patchelf executable.
    #[arg(long, default_value = "patchelf")]
    pub patchelf: PathBuf,
}
