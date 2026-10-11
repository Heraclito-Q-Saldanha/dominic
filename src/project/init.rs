use crate::*;

use std::fs;
use std::path;

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

pub fn init_project(path: &path::Path) -> error::Result<()> {
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
	fs::write(&path.join(".gitignore"), "/target\n/dist")?;

	Ok(())
}
