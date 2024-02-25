use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::{
    borrow::Cow,
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use wasm_encoder::{ComponentSection, CustomSection};
use wasmparser::Payload;

const MANIFEST_SECTION: &str = "manifest-v0";

#[derive(Args)]
struct AddManifest {
    manifest_path: PathBuf,
    module_path: PathBuf,
    #[arg(long, short)]
    output: PathBuf,
}

#[derive(Args)]
struct ReadManifest {
    path: PathBuf,
}

#[derive(Subcommand)]
enum ToolSubcommand {
    /// Add a manifest to a WASM component
    AddManifest(AddManifest),

    /// Read the manifest from a WASM component
    ReadManifest(ReadManifest),
}

#[derive(Parser)]
#[command(about, version)]
pub struct ToolArgs {
    #[clap(subcommand)]
    command: ToolSubcommand,
}

fn read_manifest(blob: &[u8]) -> Result<()> {
    let mut payload_iter = wasmparser::Parser::new(0).parse_all(blob);

    let mut data = None;
    while let Some(payload) = payload_iter.next().transpose()? {
        let Payload::CustomSection(section) = payload else {
            continue;
        };
        if section.name() != MANIFEST_SECTION {
            continue;
        }

        data = Some(section.data());
        break;
    }

    let Some(data) = data else {
        bail!("WASM blob doesn't have a manifest");
    };

    let manifest: Value = serde_json::from_slice(data)?;
    let prettified = serde_json::to_string_pretty(&manifest)?;

    println!("{prettified}");

    Ok(())
}

fn write_manifest(manifest: &[u8], module_path: &Path) -> Result<()> {
    let custom_section = CustomSection {
        name: Cow::Borrowed(MANIFEST_SECTION),
        data: Cow::Borrowed(manifest),
    };

    let mut buffer = Vec::new();
    custom_section.append_to_component(&mut buffer);

    let mut file = File::options().append(true).open(module_path)?;
    file.write_all(&buffer)?;

    Ok(())
}

fn main() -> Result<()> {
    let args = ToolArgs::parse();

    match args.command {
        ToolSubcommand::AddManifest(args) => {
            let manifest = fs::read(args.manifest_path)?;
            fs::copy(&args.module_path, &args.output)?;
            write_manifest(&manifest, &args.output)?;
        }
        ToolSubcommand::ReadManifest(args) => {
            let data = fs::read(args.path)?;
            read_manifest(&data)?;
        }
    }

    Ok(())
}
