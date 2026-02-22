use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sc_ast::ScSyntax;
use sc_desugar::desugar_module;
use sc_parser::parse_sugarcube;
use swc_common::source_map::DefaultSourceMapGenConfig;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter, Node};

#[derive(Parser)]
#[command(name = "sc", about = "sugarcube — TypeScript with extended syntax")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse, desugar, and emit standard TypeScript.
    Preprocess {
        /// Input .ts/.tsx file.
        input: PathBuf,
        /// Output file (stdout if omitted).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Treat the file as TSX.
        #[arg(long)]
        tsx: bool,
        /// Generate a source map.
        #[arg(long)]
        source_map: bool,
    },
    /// Parse the file and report any syntax errors.
    Check {
        input: PathBuf,
        #[arg(long)]
        tsx: bool,
    },
    /// Parse and dump the AST as JSON.
    Parse {
        input: PathBuf,
        #[arg(long)]
        ast: bool,
        #[arg(long)]
        tsx: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Preprocess {
            input,
            output,
            tsx,
            source_map,
        } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            let tsx_opt = if tsx { Some(true) } else { None };
            let parsed = parse_sugarcube(&source, &filename, &syntax, tsx_opt)?;
            let module = desugar_module(parsed.module);

            let mut buf = Vec::new();
            let mut srcmap_buf = if source_map { Some(vec![]) } else { None };
            {
                let writer = JsWriter::new(
                    parsed.source_map.clone(),
                    "\n",
                    &mut buf,
                    srcmap_buf.as_mut(),
                );
                let mut emitter = Emitter {
                    cfg: swc_ecma_codegen::Config::default()
                        .with_target(swc_ecma_ast::EsVersion::latest()),
                    cm: parsed.source_map.clone(),
                    comments: None,
                    wr: writer,
                };
                module.emit_with(&mut emitter)?;
            }

            let output_str = String::from_utf8(buf)?;

            match &output {
                Some(path) => std::fs::write(path, &output_str)?,
                None => print!("{output_str}"),
            }

            if source_map {
                if let Some(srcmap_data) = srcmap_buf {
                    let srcmap = parsed
                        .source_map
                        .build_source_map(&srcmap_data, None, DefaultSourceMapGenConfig);
                    let mut srcmap_json = vec![];
                    srcmap
                        .to_writer(&mut srcmap_json)
                        .context("failed to serialize source map")?;
                    let srcmap_str = String::from_utf8(srcmap_json)?;

                    let map_path = match &output {
                        Some(path) => format!("{}.map", path.display()),
                        None => format!("{filename}.map"),
                    };
                    std::fs::write(&map_path, &srcmap_str)?;
                    eprintln!("Source map written to {map_path}");
                }
            }
        }
        Commands::Check { input, tsx } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            let tsx_opt = if tsx { Some(true) } else { None };
            parse_sugarcube(&source, &filename, &syntax, tsx_opt)?;
            eprintln!("OK: {filename}");
        }
        Commands::Parse { input, ast, tsx } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            let tsx_opt = if tsx { Some(true) } else { None };
            let parsed = parse_sugarcube(&source, &filename, &syntax, tsx_opt)?;

            if ast {
                let json = serde_json::to_string_pretty(&parsed.module)?;
                println!("{json}");
            } else {
                println!("{:#?}", parsed.module);
            }
        }
    }

    Ok(())
}
