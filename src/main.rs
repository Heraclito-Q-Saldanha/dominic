pub mod error;
pub mod project;
pub mod transpiler;

use clap::Parser;
use clap::Subcommand;

use std::env;
use std::path;

pub const DEFAULT_EXPAND_PATH: &'static str = "target/expand";
pub const DEFAULT_BUILD_PATH: &'static str = "dist";

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo")]
enum Cargo {
	#[command(subcommand)]
	Dominic(Dominic),
}

#[derive(Subcommand)]
enum Dominic {
	New {
		name: String,
	},
	Init,
	Run {
		#[arg(short, long, default_value_t = 8080)]
		port: u16,
	},
	Build {
		#[arg(short, long, default_value = DEFAULT_BUILD_PATH)]
		out: path::PathBuf,
	},
	Expand {
		#[arg(short, long, default_value = DEFAULT_EXPAND_PATH)]
		out: path::PathBuf,
	},
}

fn main() -> error::Result<()> {
	simple_logger::init().unwrap();

	let Cargo::Dominic(command) = Cargo::parse();

	match command {
		Dominic::New { name } => {
			log::info!("Creating project: {}", name);

			project::create_project(&name)
		}
		Dominic::Init => {
			log::info!("Initializing project");

			let Ok(path) = env::current_dir() else {
				return Err(error::Error::InvalidFilePath);
			};

			project::init_project(&path)
		}
		Dominic::Run { port } => {
			log::info!("Running on port: {}", port);

			project::run_project(port)
		}
		Dominic::Build { out } => {
			log::info!("Building project to output: {:?}", out);

			project::build_project(&out)
		}
		Dominic::Expand { out } => {
			log::info!("Expanding to output: {:?}", out);

			project::expand_project(&out)
		}
	}
}
