pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
	Io(std::io::Error),
	Html(lol_html::errors::RewritingError),
	Syn(syn::Error),
	WalkDir(walkdir::Error),
	Toml(toml_edit::TomlError),
	Cargo,
	InvalidFilePath,
	InvalidManifest,
}

impl From<toml_edit::TomlError> for Error {
	fn from(value: toml_edit::TomlError) -> Self {
		Self::Toml(value)
	}
}

impl From<syn::Error> for Error {
	#[inline(always)]
	fn from(value: syn::Error) -> Self {
		Self::Syn(value)
	}
}

impl From<lol_html::errors::RewritingError> for Error {
	#[inline(always)]
	fn from(value: lol_html::errors::RewritingError) -> Self {
		Self::Html(value)
	}
}

impl From<std::io::Error> for Error {
	#[inline(always)]
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<walkdir::Error> for Error {
	#[inline(always)]
	fn from(value: walkdir::Error) -> Self {
		Self::WalkDir(value)
	}
}
