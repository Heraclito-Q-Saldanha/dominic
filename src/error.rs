pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
	Io(std::io::Error),
	Html(lol_html::errors::RewritingError),
	Rust(syn::Error),
	InvalidFilePath,
}

impl From<syn::Error> for Error {
	fn from(value: syn::Error) -> Self {
		Self::Rust(value)
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
