pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
	Io(std::io::Error),
	InvalidFilePath,
}

impl From<std::io::Error> for Error {
	#[inline(always)]
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}
