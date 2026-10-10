mod error;
mod transpiler;

use clap::Parser;
use clap::Subcommand;

use std::env;
use std::fs;
use std::io;
use std::path;
use std::process;

const DEFAULT_EXPAND_PATH: &'static str = "target/expand";
const DEFAULT_BUILD_PATH: &'static str = "dist";

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

const DEFAULT_PAGE: &'static str = r#"<script>
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
		Dominic::Build { out } => {
			log::info!("Building project to output: {:?}", out);

			build_project(&out)
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

	fs::create_dir_all(&path.join("src").join("routes"))?;

	let mut manifest = toml_edit::DocumentMut::new();

	manifest["package"] = toml_edit::Item::Table(toml_edit::Table::new());

	manifest["package"]["name"] = toml_edit::value(name);
	manifest["package"]["version"] = toml_edit::value("0.1.0");
	manifest["package"]["edition"] = toml_edit::value("2024");

	manifest["dependencies"] = toml_edit::Item::Table(toml_edit::Table::default());

	fs::write(&path.join("Cargo.toml"), manifest.to_string())?;
	fs::write(&path.join("src").join("routes").join("+page.minic"), DEFAULT_PAGE)?;
	fs::write(&path.join(".gitignore"), "/target")?;

	Ok(())
}

fn run_project(port: u16) -> error::Result<()> {
	Ok(())
}

fn build_project(output: &path::Path) -> error::Result<()> {
	let expand_path = path::PathBuf::from(DEFAULT_EXPAND_PATH);

	expand_project(&expand_path)?;

	let manifest_path = expand_path.join("Cargo.toml");

	let mut manifest = fs::read_to_string(&manifest_path)?.parse::<toml_edit::DocumentMut>()?;
	let name = manifest["package"]["name"].as_str().ok_or(error::Error::InvalidManifest)?.replace('-', "_");

	if !manifest.contains_key("workspace") {
		manifest["workspace"] = toml_edit::Item::Table(toml_edit::Table::new());
	}

	fs::write(&manifest_path, manifest.to_string())?;

	let status = process::Command::new("cargo")
		.current_dir(&expand_path)
		.args(["rustc", "--release", "--lib", "--crate-type", "cdylib", "--target", "wasm32-unknown-unknown", "--target-dir", "target"])
		.status()?;

	if !status.success() {
		return Err(error::Error::Cargo);
	}

	let release_dir = expand_path.join("target").join("wasm32-unknown-unknown").join("release").join(format!("{name}.wasm"));
	let routes_path = expand_path.join("src").join("routes");

	fs::create_dir_all(output)?;
	fs::copy(&release_dir, output.join("index.wasm"))?;

	for path in walkdir::WalkDir::new(&routes_path) {
		let path = path?.path().to_path_buf();
		let name = path.file_name().ok_or(error::Error::InvalidFilePath)?.to_string_lossy();

		if !path.is_file() || name != "index.html" {
			continue;
		}

		let relative = path.strip_prefix(&routes_path).map_err(|_| error::Error::InvalidFilePath)?;
		let destination = output.join(relative);
		let parent = destination.parent().ok_or(error::Error::InvalidFilePath)?;

		fs::create_dir_all(parent)?;
		fs::copy(path, destination)?;
	}

	Ok(())
}

fn expand_project(output: &path::Path) -> error::Result<()> {
	let src_path = path::PathBuf::from("src");
	let routes_path = src_path.join("routes");

	fs::create_dir_all(output.join(&routes_path))?;

	fs::copy("Cargo.toml", output.join("Cargo.toml"))?;
	fs::write(output.join(src_path.join("lib.rs")), "mod routes;")?;

	for path in walkdir::WalkDir::new(&routes_path) {
		let path = path?.path().to_path_buf();

		let Some(name) = path.file_name() else {
			continue;
		};

		let Some(parent) = path.parent() else {
			continue;
		};

		let code = if path.is_dir() {
			fs::create_dir_all(output.join(&path))?;
			fs::File::create(output.join(&path).join("mod.rs"))?;

			if parent == src_path {
				continue;
			}

			format!("mod {};", name.to_string_lossy())
		} else {
			if name != "+page.minic" {
				continue;
			}

			let input = fs::read_to_string(&path)?;

			let (code, html) = transpiler::transpile(&input)?;

			fs::write(output.join(&parent).join("index.html"), html)?;

			code
		};

		let mut module = fs::OpenOptions::new().append(true).open(output.join(parent).join("mod.rs"))?;

		io::Write::write_all(&mut module, code.as_bytes())?;
		io::Write::write_all(&mut module, b"\n")?;
	}

	Ok(())
}
