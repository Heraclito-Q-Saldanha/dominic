pub mod error;
pub mod project;
pub mod transpiler;

use std::env;
use std::path;

use clap::Parser;

pub const DEFAULT_EXPAND_PATH: &'static str = "target";
pub const DEFAULT_BUILD_PATH: &'static str = "target/dist";

#[derive(Parser)]
enum Cli {
	New {
		name: String,
	},
	Init,
	Clean,
	Run {
		#[arg(short, long, default_value_t = 8080)]
		port: u16,
		#[arg(short, long, default_value = "127.0.0.1")]
		address: String,
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
	simple_logger::init_with_level(log::Level::Info).unwrap();

	let command = Cli::parse();

	match command {
		Cli::New { name } => {
			log::info!("Creating project: {}", name);

			project::create_project(&name)
		}
		Cli::Init => {
			log::info!("Initializing project");

			let Ok(path) = env::current_dir() else {
				return Err(error::Error::InvalidFilePath);
			};

			project::init_project(&path)
		}
		Cli::Clean => {
			log::info!("Cleaning project");

			project::clean_project()
		}
		Cli::Run { port, address } => {
			log::info!("Running on http://{}:{}", address, port);

			project::run_project(&address, port)
		}
		Cli::Build { out } => {
			log::info!("Building project to output: {:?}", out);

			project::build_project(&out)
		}
		Cli::Expand { out } => {
			log::info!("Expanding to output: {:?}", out);

			project::expand_project(&out)
		}
	}
}
