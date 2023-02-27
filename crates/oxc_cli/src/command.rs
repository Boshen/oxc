use clap::{builder::ValueParser, Arg, Command as ClapCommand};

#[derive(Debug)]
pub struct Command {
    inner: ClapCommand,
}

impl Default for Command {
    fn default() -> Self {
        Self::new()
    }
}

impl Command {
    #[must_use]
    pub fn new() -> Self {
        let inner = ClapCommand::new("oxc")
            .bin_name("oxc")
            .version("alpha")
            .author("Boshen")
            .about("The JavaScript Oxidation Compiler")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(Self::lint_subcommand());
        Self { inner }
    }

    fn lint_subcommand() -> ClapCommand {
        ClapCommand::new("lint")
            .alias("check")
            .about("Lint this repository.")
            .arg_required_else_help(true)
            .arg(
              // This flag follows ESLint's --quiet behavior as documented here: https://eslint.org/docs/latest/use/command-line-interface#--quiet
              // Note that it does not disable ALL diagnostics, only Warning diagnostics
              Arg::new("quiet")
                .long("quiet")
                .required(false)
                .action(clap::ArgAction::SetTrue)
                .help("This option allows you to disable reporting on warnings. If you enable this option, only errors are reported by oxc_lint.")
            )
            .arg(
                Arg::new("path")
                    .value_name("PATH")
                    .num_args(1..)
                    .required(true)
                    .help("File or Directory paths to scan. Directories are scanned recursively.")
                    .value_parser(ValueParser::path_buf()),
            )
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn build(self) -> ClapCommand {
        self.inner
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::Command;

    #[test]
    fn verify_command() {
        Command::new().build().debug_assert();
    }

    #[test]
    fn test_lint_path() {
        let arg = "oxc lint .";
        let matches = Command::new().build().try_get_matches_from(arg.split(' ')).unwrap();
        let matches = matches.subcommand_matches("lint");
        assert!(matches.is_some());
        assert_eq!(matches.unwrap().get_one::<PathBuf>("path"), Some(&PathBuf::from(".")));
    }

    #[test]
    fn test_lint_multiple_paths() {
        let arg = "oxc lint foo bar baz";
        let matches = Command::new().build().try_get_matches_from(arg.split(' ')).unwrap();
        let matches = matches.subcommand_matches("lint");
        assert!(matches.is_some());
        assert_eq!(
            matches.unwrap().get_many::<PathBuf>("path").unwrap().collect::<Vec<_>>(),
            [&PathBuf::from("foo"), &PathBuf::from("bar"), &PathBuf::from("baz")]
        );
    }

    #[test]
    fn test_check_path() {
        let arg = "oxc check /path/to/dir";
        let matches = Command::new().build().try_get_matches_from(arg.split(' ')).unwrap();
        let matches = matches.subcommand_matches("lint");
        assert!(matches.is_some());
        assert_eq!(
            matches.unwrap().get_one::<PathBuf>("path"),
            Some(&PathBuf::from("/path/to/dir"))
        );
    }
}
