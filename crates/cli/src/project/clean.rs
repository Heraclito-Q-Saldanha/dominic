use crate::*;

use std::fs;
use std::io;

pub fn clean_project() -> error::Result<()> {
	for directory in [DEFAULT_BUILD_PATH, DEFAULT_EXPAND_PATH] {
		match fs::remove_dir_all(directory) {
			Ok(()) => {}
			Err(error) if error.kind() == io::ErrorKind::NotFound => {}
			Err(error) => return Err(error.into()),
		}
	}

	Ok(())
}
