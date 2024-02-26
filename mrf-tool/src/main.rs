use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::{
    borrow::Cow,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use wasm_encoder::{ComponentSection, CustomSection};
use wasmparser::{CustomSectionReader, Payload};

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
    module_path: PathBuf,
}

#[derive(Args)]
struct RemoveManifest {
    module_path: PathBuf,
    #[arg(long, short)]
    output: PathBuf,
}

#[derive(Args)]
struct ValidateModule {
    module_path: PathBuf,
}

#[derive(Subcommand)]
enum ToolSubcommand {
    /// Add a manifest to a WASM component
    AddManifest(AddManifest),

    /// Read the manifest from a WASM component
    ReadManifest(ReadManifest),

    /// Remove the manifest from a WASM component
    RemoveManifest(RemoveManifest),

    /// Validate a WASM module
    ValidateModule(ValidateModule),
}

#[derive(Parser)]
#[command(about, version)]
pub struct ToolArgs {
    #[clap(subcommand)]
    command: ToolSubcommand,
}

fn find_manifest_section(blob: &[u8]) -> Result<CustomSectionReader<'_>> {
    let mut payload_iter = wasmparser::Parser::new(0).parse_all(blob);
    let mut manifest_section = None;
    while let Some(payload) = payload_iter.next().transpose()? {
        let Payload::CustomSection(section) = payload else {
            continue;
        };
        if section.name() != MANIFEST_SECTION {
            continue;
        }

        manifest_section = Some(section);
        break;
    }

    manifest_section.ok_or_else(|| anyhow!("no manifest section found"))
}

fn read_manifest(blob: &[u8]) -> Result<()> {
    let manifest_section = find_manifest_section(blob)?;
    let manifest: Value = serde_json::from_slice(manifest_section.data())?;
    let prettified = serde_json::to_string_pretty(&manifest)?;

    println!("{prettified}");

    Ok(())
}

fn remove_manifest(module_path: &Path, output_path: &Path) -> Result<()> {
    let blob = fs::read(module_path)?;
    let manifest_section = find_manifest_section(&blob)?;
    let manifest_range = manifest_section.range();

    let mut module_file = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_path)?;

    // Check the size of the LEB128 encoded integer
    let length_size =
        leb128::write::unsigned(&mut io::sink(), manifest_section.data().len() as u64).unwrap();
    let start_offset = 1 + length_size; // 1 byte for the section identifier, N bytes for the length of the section

    module_file.write_all(&blob[..manifest_range.start - start_offset])?;
    module_file.write_all(&blob[manifest_range.end..])?;

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
            let data = fs::read(args.module_path)?;
            read_manifest(&data)?;
        }
        ToolSubcommand::RemoveManifest(args) => {
            remove_manifest(&args.module_path, &args.output)?;
        }
        ToolSubcommand::ValidateModule(args) => {
            let data = fs::read(args.module_path)?;
            wasmparser::validate(&data)?;
        }
    }

    Ok(())
}
