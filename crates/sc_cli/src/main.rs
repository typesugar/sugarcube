use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sc_ast::ScSyntax;
use sc_desugar::desugar_module;
use sc_parser::parse_sugarcube;
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
            tsx: _,
            source_map: _,
        } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            let parsed = parse_sugarcube(&source, &filename, &syntax)?;
            let module = desugar_module(parsed.module);

            let mut buf = Vec::new();
            {
                let writer = JsWriter::new(parsed.source_map.clone(), "\n", &mut buf, None);
                let mut emitter = Emitter {
                    cfg: swc_ecma_codegen::Config::default().with_target(swc_ecma_ast::EsVersion::latest()),
                    cm: parsed.source_map,
                    comments: None,
                    wr: writer,
                };
                module.emit_with(&mut emitter)?;
            }

            let output_str = String::from_utf8(buf)?;

            match output {
                Some(path) => std::fs::write(path, &output_str)?,
                None => print!("{output_str}"),
            }
        }
        Commands::Check { input, tsx: _ } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            parse_sugarcube(&source, &filename, &syntax)?;
            eprintln!("OK: {filename}");
        }
        Commands::Parse {
            input,
            ast,
            tsx: _,
        } => {
            let source = std::fs::read_to_string(&input)?;
            let filename = input.display().to_string();
            let syntax = ScSyntax::default();

            let parsed = parse_sugarcube(&source, &filename, &syntax)?;

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
