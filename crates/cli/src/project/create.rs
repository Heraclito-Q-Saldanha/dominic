use crate::*;

use std::fs;
use std::path;

pub fn create_project(name: &str) -> error::Result<()> {
	let path = path::Path::new(name);

	fs::create_dir_all(&path)?;

	project::init_project(&path)
}
