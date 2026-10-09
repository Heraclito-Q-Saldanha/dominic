mod error;

use clap::Args;
use clap::Parser;
use clap::Subcommand;

use std::fs;
use std::path;

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo")]
enum Cargo {
	Dominic(Dominic),
}

#[derive(Args)]
#[command(version, about)]
struct Dominic {
	#[command(subcommand)]
	command: Command,
}

#[derive(Subcommand)]
enum Command {
	New {
		name: String,
	},
	Init,
	Run {
		#[arg(short, long, default_value_t = 8080)]
		port: u16,
	},
	Build,
	Expand {
		#[arg(short, long, default_value = "target/wk-expand")]
		out: path::PathBuf,
	},
}

fn main() -> error::Result<()> {
	let Cargo::Dominic(Dominic { command }) = Cargo::parse();

	match command {
		Command::New { name } => create_project(&name),
		Command::Init => init_project(),
		Command::Run { port } => run_project(port),
		Command::Build => build_project(),
		Command::Expand { out } => expand_project(&out),
	}
}

fn create_project(name: &str) -> error::Result<()> {
	log::info!("Creating project: {}", name);

	fs::create_dir_all(name)?;

	Ok(())
}

fn init_project() -> error::Result<()> {
	log::info!("Initializing project");

	Ok(())
}

fn run_project(port: u16) -> error::Result<()> {
	log::info!("Running on port: {}", port);

	Ok(())
}

fn build_project() -> error::Result<()> {
	log::info!("Building project");

	Ok(())
}

fn expand_project(out: &path::PathBuf) -> error::Result<()> {
	log::info!("Expanding to output: {:?}", out);

	Ok(())
}
