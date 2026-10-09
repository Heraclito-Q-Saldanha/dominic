mod error;

use clap::Parser;
use clap::Subcommand;

use std::fs;
use std::path;

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
	Build,
	Expand {
		#[arg(short, long, default_value = "target/wk-expand")]
		out: path::PathBuf,
	},
}

const DEFAULT_INDEX: &'static str = r#"<script>
	let mut counter = 0;

	pub fn add(){
		counter += 1;
	}
</script>
<h1>Welcome to my project</h1>
<button onclick="add">Add</button>
<p>{counter}</p>
"#;

fn main() -> error::Result<()> {
	simple_logger::init().unwrap();

	let Cargo::Dominic(command) = Cargo::parse();

	match command {
		Dominic::New { name } => create_project(&name),
		Dominic::Init => init_project(),
		Dominic::Run { port } => run_project(port),
		Dominic::Build => build_project(),
		Dominic::Expand { out } => expand_project(&out),
	}
}

fn create_project(name: &str) -> error::Result<()> {
	log::info!("Creating project: {}", name);

	let path = path::PathBuf::from(name);

	fs::create_dir_all(&path)?;
	fs::create_dir_all(&path.join("src"))?;
	fs::create_dir_all(&path.join("src").join("routes"))?;

	let mut manifest = toml_edit::DocumentMut::new();

	manifest["package"] = toml_edit::Item::Table(toml_edit::Table::new());

	manifest["package"]["name"] = toml_edit::value(name);
	manifest["package"]["version"] = toml_edit::value("0.1.0");
	manifest["package"]["edition"] = toml_edit::value("2024");

	manifest["dependencies"] = toml_edit::Item::Table(toml_edit::Table::default());

	fs::write(&path.join("Cargo.toml"), manifest.to_string())?;
	fs::write(&path.join("src").join("routes").join("+page.minic"), DEFAULT_INDEX)?;

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
