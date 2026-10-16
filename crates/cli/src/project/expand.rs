use crate::*;

use std::fs;
use std::io;
use std::path;
use std::process;

pub fn expand_project(output: &path::Path) -> error::Result<()> {
	let src_path = path::Path::new("src");
	let routes_path = src_path.join("routes");
	let manifest_path = output.join("Cargo.toml");

	fs::create_dir_all(output.join(&routes_path))?;
	fs::write(output.join(src_path.join("lib.rs")), "mod routes;")?;

	let mut manifest = fs::read_to_string("Cargo.toml")?.parse::<toml_edit::DocumentMut>()?;

	if !manifest.contains_key("workspace") {
		manifest["workspace"] = toml_edit::Item::Table(toml_edit::Table::new());
	}

	fs::write(&manifest_path, manifest.to_string())?;

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

			let (html, code) = transpiler::transpile(&input)?;

			let content = format!(
				r#"<!DOCTYPE html>
<html>
	<head>
		<meta charset="UTF-8">
	</head>
	<body>
		<script type="module">
			import * as script from "/index.js";

			const start = async () => {{
				await script.default({{ module_or_path: "/index_bg.wasm" }});
				window._dominic_ = script;
				window._dominic_.{}();
			}};
			if (document.readyState === "complete") {{
				start();
			}} else {{
				window.addEventListener("load", start, {{ once: true }});
			}}
		</script>
		{}
	</body>
</html>
"#,
				code.init_fn, html.html
			);

			fs::write(output.join(&parent).join("index.html"), content)?;

			code.code
		};

		let mut module = fs::OpenOptions::new().append(true).open(output.join(parent).join("mod.rs"))?;

		io::Write::write_all(&mut module, code.as_bytes())?;
		io::Write::write_all(&mut module, b"\n")?;
	}

	let status = process::Command::new("cargo").current_dir(output).arg("fmt").status()?;

	if !status.success() {
		return Err(error::Error::CargoFmt(status));
	}

	Ok(())
}
