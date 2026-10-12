use crate::*;

use std::fs;
use std::io;
use std::path;

pub fn expand_project(output: &path::Path) -> error::Result<()> {
	let src_path = path::Path::new("src");
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
				window.__dominic = script;
				window.__dominic.{}();
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

	Ok(())
}
