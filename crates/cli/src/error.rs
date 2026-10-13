pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Failed to rewrite page HTML: {0}")]
	Html(#[from] lol_html::errors::RewritingError),
	#[error("Invalid Rust code in <script>: {0}")]
	Syn(#[from] syn::Error),
	#[error("Failed to traverse project directories: {0}")]
	WalkDir(#[from] walkdir::Error),
	#[error("Failed to parse Cargo.toml: {0}")]
	Toml(#[from] toml_edit::TomlError),
	#[error("Failed to watch project files: {0}")]
	Notify(#[from] notify::Error),
	#[error("Failed to generate WebAssembly bindings: {0}")]
	Anyhow(#[from] anyhow::Error),
	#[error("Cargo failed to compile the project;")]
	Cargo(std::process::ExitStatus),
	#[error("Invalid project file or directory path")]
	InvalidFilePath,
	#[error("Invalid Cargo.toml")]
	InvalidManifest,
}
