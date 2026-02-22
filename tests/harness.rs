//! Golden-file test harness for sugarcube.
//!
//! Discovers `.input.ts` files under `tests/fixtures/`, runs the sugarcube
//! pipeline (parse → desugar → codegen), and compares output against the
//! corresponding `.expected.ts` file.
//!
//! Set `SC_UPDATE_FIXTURES=1` to overwrite expected files with actual output.

use std::path::{Path, PathBuf};

use anyhow::Result;
use sc_ast::ScSyntax;
use sc_desugar::desugar_module;
use sc_parser::parse_sugarcube;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter, Node};

fn fixtures_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/sc_test/, so go up two levels to workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

fn collect_input_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }
    for entry in walkdir(dir) {
        if entry.extension().is_some_and(|e| e == "ts")
            && entry
                .file_name()
                .unwrap()
                .to_str()
                .is_some_and(|n| n.ends_with(".input.ts"))
        {
            files.push(entry);
        }
    }
    files.sort();
    files
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}

fn run_pipeline(source: &str, filename: &str) -> Result<String> {
    let syntax = ScSyntax::default();
    let parsed = parse_sugarcube(source, filename, &syntax)?;
    let module = desugar_module(parsed.module);

    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(parsed.source_map.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config::default()
                .with_target(swc_ecma_ast::EsVersion::latest()),
            cm: parsed.source_map,
            comments: None,
            wr: writer,
        };
        module.emit_with(&mut emitter)?;
    }

    Ok(String::from_utf8(buf)?)
}

fn verify_valid_typescript(output: &str, filename: &str) -> Result<()> {
    let syntax = ScSyntax {
        pipeline: false,
        cons: false,
        hkt: false,
    };
    parse_sugarcube(output, filename, &syntax)?;
    Ok(())
}

#[test]
fn golden_file_tests() {
    let fixtures = fixtures_dir();
    let input_files = collect_input_files(&fixtures);

    assert!(
        !input_files.is_empty(),
        "No test fixtures found in {}",
        fixtures.display()
    );

    let update_mode = std::env::var("SC_UPDATE_FIXTURES").is_ok();
    let mut failures = Vec::new();

    for input_path in &input_files {
        let expected_path = input_path
            .to_str()
            .unwrap()
            .replace(".input.ts", ".expected.ts");
        let expected_path = PathBuf::from(&expected_path);

        let test_name = input_path
            .strip_prefix(&fixtures)
            .unwrap()
            .display()
            .to_string();

        let source = match std::fs::read_to_string(input_path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{test_name}: failed to read input: {e}"));
                continue;
            }
        };

        let filename = input_path.display().to_string();
        let actual = match run_pipeline(&source, &filename) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{test_name}: pipeline failed: {e}"));
                continue;
            }
        };

        if update_mode {
            if let Err(e) = std::fs::write(&expected_path, &actual) {
                failures.push(format!("{test_name}: failed to write expected: {e}"));
            }
            continue;
        }

        if !expected_path.exists() {
            failures.push(format!(
                "{test_name}: missing expected file: {}",
                expected_path.display()
            ));
            continue;
        }

        let expected = match std::fs::read_to_string(&expected_path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{test_name}: failed to read expected: {e}"));
                continue;
            }
        };
        if actual.trim() != expected.trim() {
            failures.push(format!(
                "{test_name}: output mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
                expected.trim(),
                actual.trim()
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} golden test(s) failed:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}

#[test]
fn roundtrip_tests() {
    let fixtures = fixtures_dir().join("roundtrip");
    let input_files = collect_input_files(&fixtures);

    let mut failures = Vec::new();

    for input_path in &input_files {
        let test_name = input_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let source = match std::fs::read_to_string(input_path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{test_name}: failed to read: {e}"));
                continue;
            }
        };

        let filename = input_path.display().to_string();
        let output = match run_pipeline(&source, &filename) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{test_name}: pipeline failed: {e}"));
                continue;
            }
        };

        if let Err(e) = verify_valid_typescript(&output, &format!("{test_name}.output")) {
            failures.push(format!(
                "{test_name}: output is not valid TypeScript: {e}\n--- output ---\n{}",
                output.trim()
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} roundtrip test(s) failed:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
