use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;
use crate::cli::Cli;

pub struct Completions;

impl Completions {
    pub fn generate_to_stdout(shell: Shell) {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut io::stdout());
    }
}
