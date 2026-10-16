use crate::*;

use std::fs;
use std::path;

const DEFAULT_PAGE: &'static str = r#"<script>
	let mut counter = state!(0);

	pub fn add(){
		counter += 1;
	}
</script>
<h1>Welcome to my project</h1>
<button onclick={add}>Add</button>
<p>{counter}</p>
"#;

const DEFAULT_CARGO_TOML: &'static str = r#"[package]
name = "PACKAGE_NAME"
version = "0.1.0"
edition = "2024"

[dependencies]
dominic-runtime = "0.1.0"

[profile.release]
opt-level = "z"
strip = true
lto = true
"#;

pub fn init_project(path: &path::Path) -> error::Result<()> {
	let Some(name) = path.file_name() else {
		return Err(error::Error::InvalidFilePath);
	};

	let name = name.to_string_lossy();
	let manifest = DEFAULT_CARGO_TOML.replace("PACKAGE_NAME", &name);

	fs::create_dir_all(&path.join("src").join("routes"))?;

	fs::write(&path.join("Cargo.toml"), manifest)?;
	fs::write(&path.join("src").join("routes").join("+page.minic"), DEFAULT_PAGE)?;
	fs::write(&path.join(".gitignore"), "/target\n")?;

	Ok(())
}
