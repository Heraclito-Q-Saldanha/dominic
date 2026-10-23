use crate::*;

use std::fs;
use std::io;
use std::path;

const PAGE_TEMPLATE: &'static str = include_str!("../page_template.html");

pub fn expand_project(output: &path::Path) -> error::Result<()> {
	let src_path = path::Path::new("src");
	let routes_path = src_path.join("routes");
	let manifest_path = output.join("Cargo.toml");

	fs::create_dir_all(output.join(&routes_path))?;
	fs::File::create(output.join(src_path.join("lib.rs")))?;

	let mut manifest = fs::read_to_string("Cargo.toml")?.parse::<toml_edit::DocumentMut>()?;

	if !manifest.contains_key("workspace") {
		manifest["workspace"] = toml_edit::Item::Table(toml_edit::Table::new());
	}

	fs::write(&manifest_path, manifest.to_string())?;

	for path in walkdir::WalkDir::new(&src_path) {
		let path = path?.path().to_path_buf();

		if path == src_path {
			continue;
		}

		let Some(name) = path.file_name() else {
			continue;
		};

		let Some(parent) = path.parent() else {
			continue;
		};

		let code_file_name = if parent == src_path { "lib.rs" } else { "mod.rs" };

		let code_path = output.join(&parent).join(code_file_name);
		let html_path = output.join(&parent).join("index.html");

		let mut code_file = fs::OpenOptions::new().append(true).open(code_path)?;

		if path.is_dir() {
			fs::create_dir_all(output.join(&path))?;
			fs::File::create(output.join(&path).join("mod.rs"))?;

			let code = format!("pub mod {};", name.to_string_lossy());

			io::Write::write_all(&mut code_file, code.as_bytes())?;
			io::Write::write_all(&mut code_file, b"\n")?;
		} else {
			let input = fs::read_to_string(&path)?;

			if name == "+page.minic" {
				let (html, code) = transpiler::transpile_page(&input)?;

				let code_content = code.code;
				let html_content = PAGE_TEMPLATE.replace("{PAGE_ENTRY_POINT}", &code.init_fn).replace("{PAGE_BODY}", &html.html);

				let mut html_file = fs::File::create(html_path)?;

				io::Write::write_all(&mut html_file, html_content.as_bytes())?;
				io::Write::write_all(&mut code_file, code_content.as_bytes())?;
			} else {
				let code_content = transpiler::transpile_component(&input)?;

				io::Write::write_all(&mut code_file, code_content.as_bytes())?;
			}
		};
	}

	Ok(())
}
