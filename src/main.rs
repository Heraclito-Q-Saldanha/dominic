mod error;
mod transpiler;

use clap::Parser;
use clap::Subcommand;

use std::env;
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
		Dominic::New { name } => {
			log::info!("Creating project: {}", name);

			create_project(&name)
		}
		Dominic::Init => {
			log::info!("Initializing project");

			let Ok(path) = env::current_dir() else {
				return Err(error::Error::InvalidFilePath);
			};

			init_project(&path)
		}
		Dominic::Run { port } => {
			log::info!("Running on port: {}", port);

			run_project(port)
		}
		Dominic::Build => {
			log::info!("Building project");

			build_project()
		}
		Dominic::Expand { out } => {
			log::info!("Expanding to output: {:?}", out);

			expand_project(&out)
		}
	}
}

fn create_project(name: &str) -> error::Result<()> {
	let path = path::PathBuf::from(name);

	fs::create_dir_all(&path)?;

	init_project(&path)
}

fn init_project(path: &path::Path) -> error::Result<()> {
	let Some(name) = path.file_name() else {
		return Err(error::Error::InvalidFilePath);
	};

	let name = name.to_string_lossy().to_string();

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

fn run_project(port: u16) -> error::Result<()> {
	Ok(())
}

fn build_project() -> error::Result<()> {
	Ok(())
}

fn expand_project(out: &path::PathBuf) -> error::Result<()> {
	let fuu = transpiler::split_code(
		r#"
		<script>
			let mut counter = 0;
		</script>
		<p>jujuba</p>
	"#,
	)?;

	dbg!(fuu);

	Ok(())
}
