mod command;
// mod git;
mod options;
mod result;
mod walk;

use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    rc::Rc,
    sync::{mpsc, Arc},
};

use miette::NamedSource;
use oxc_allocator::Allocator;
use oxc_ast::SourceType;
use oxc_diagnostics::{Error, Severity};
use oxc_linter::{Fixer, Linter};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;

pub use crate::{command::Command, options::CliOptions, result::CliRunResult, walk::Walk};

pub struct Cli {
    pub cli_options: CliOptions,
}

impl Cli {
    #[must_use]
    pub fn new(cli_options: CliOptions) -> Self {
        Self { cli_options }
    }

    /// # Panics
    ///
    /// * When `mpsc::channel` fails to send.
    #[must_use]
    pub fn lint(&self) -> CliRunResult {
        let now = std::time::Instant::now();

        let (tx_error, rx_error) = mpsc::channel::<Error>();
        let (tx_path, rx_path) = mpsc::channel::<Box<Path>>();

        let mut number_of_files = 0;
        rayon::join(
            || {
                let paths = self
                    .cli_options
                    .paths
                    .iter()
                    .flat_map(|path| Walk::new(path, &self.cli_options).iter())
                    .filter(|path| {
                        if self.cli_options.no_ignore {
                            return true;
                        }
                        for pattern in &self.cli_options.ignore_pattern {
                            if pattern.matches_path(path) {
                                return false;
                            }
                        }
                        true
                    });
                for path in paths {
                    number_of_files += 1;
                    tx_path.send(path).unwrap();
                }
                drop(tx_path);
            },
            move || {
                let fix = self.cli_options.fix;
                while let Ok(path) = rx_path.recv() {
                    let tx_error = tx_error.clone();
                    rayon::spawn(move || {
                        let diagnostics = Self::lint_path(&path, fix);
                        for d in diagnostics {
                            tx_error.send(d).unwrap();
                        }
                        drop(tx_error);
                    });
                }
            },
        );

        let mut buf_writer = BufWriter::new(std::io::stdout());
        let mut number_of_warnings = 0;
        let mut number_of_diagnostics = 0;

        while let Ok(diagnostic) = rx_error.recv() {
            number_of_diagnostics += 1;

            if diagnostic.severity() == Some(Severity::Warning) {
                number_of_warnings += 1;
                // The --quiet flag follows ESLint's --quiet behavior as documented here: https://eslint.org/docs/latest/use/command-line-interface#--quiet
                // Note that it does not disable ALL diagnostics, only Warning diagnostics
                if self.cli_options.quiet {
                    continue;
                }

                if let Some(max_warnings) = self.cli_options.max_warnings {
                    if number_of_warnings > max_warnings {
                        continue;
                    }
                }
            }

            buf_writer
                .write_all(format!("{diagnostic:?}").as_bytes())
                .expect("Failed to write diagnostic.");
        }

        buf_writer.flush().unwrap();

        CliRunResult::LintResult {
            duration: now.elapsed(),
            number_of_files,
            number_of_diagnostics,
            number_of_warnings,
            max_warnings_exceeded: self
                .cli_options
                .max_warnings
                .map_or(false, |max_warnings| number_of_warnings > max_warnings),
        }
    }

    fn lint_path(path: &Path, fix: bool) -> Vec<Error> {
        let source_text = fs::read_to_string(path).expect("{name} not found");
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(path).expect("incorrect {path:?}");
        let parser_source_text = source_text.clone();
        let ret = Parser::new(&allocator, &parser_source_text, source_type).parse();
        if !ret.errors.is_empty() {
            return ret.errors;
        }

        let program = allocator.alloc(ret.program);
        let trivias = Rc::new(ret.trivias);
        let semantic_ret = SemanticBuilder::new(source_type).build(program, trivias);

        if !semantic_ret.errors.is_empty() {
            return ret.errors;
        };

        let result = Linter::new().run(&Rc::new(semantic_ret.semantic), &source_text, fix);

        if result.is_empty() {
            return vec![];
        }

        let path_cow = path.to_string_lossy();
        let source = Arc::new(NamedSource::new(path_cow, source_text.clone()));

        if fix {
            let fix_result = Fixer::new(&source_text, result).fix();
            fs::write(path, fix_result.fixed_code.as_bytes())
                .unwrap_or_else(|_| panic!("{path:?} not found"));
            return fix_result
                .messages
                .into_iter()
                .map(|m| m.error.with_source_code(source.clone()))
                .collect();
        }

        result
            .into_iter()
            .map(|diagnostic| diagnostic.error.with_source_code(source.clone()))
            .collect()
    }
}
