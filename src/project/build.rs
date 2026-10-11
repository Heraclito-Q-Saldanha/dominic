use crate::*;

use std::fs;
use std::path;
use std::process;

pub fn build_project(output: &path::Path) -> error::Result<()> {
	let expand_path = path::PathBuf::from(DEFAULT_EXPAND_PATH);

	project::expand_project(&expand_path)?;

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
