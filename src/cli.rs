use crate::{utils::{self, Language}, valgrind};

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "Kiln")]
#[command(version = "0.1.1")]
#[command(about = "A modern build system for C/C++", long_about = None)]
pub struct CliCommand {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init {
        #[arg(value_enum, long, default_value = "c")]
        language: utils::Language,
    },
    New {
        proj_name: String,

        #[arg(value_enum, long, default_value = "c")]
        language: utils::Language,
    },

    // Clap doesn't provide any way to structure the syntax to be `kiln run --profile
    // So, we'll have to parse these manually.
    Build {
        profile: String,
    },
    Run {
        profile: String,
        args: Vec<String>,
        valgrind: bool,
    },
    GenHeaders,
}

impl Commands {
    pub fn new(variant: &str, profile: &str, args: Vec<String>, valgrind: bool) -> Self {
        match variant {
            "build" => Self::Build {
                profile: profile.to_string(),
            },
            "run" => Self::Run {
                profile: profile.to_string(),
                args,
                valgrind,
            },
            _ => panic!("Parameter `variant` must be one of 'build' or 'run'"),
        }
    }
}

#[allow(unused)]
fn parse_language(arg: &str) -> Result<Language, &str> {
    match arg {
        "c" => Ok(Language::C),
        "cpp" | "c++" => Ok(Language::Cpp),
        _ => {
            println!("Language `{}` is not supported", arg);
            std::process::exit(1);
        }
    }
}


