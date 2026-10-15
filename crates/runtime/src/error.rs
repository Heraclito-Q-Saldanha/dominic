pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("Window unavailable")]
	WindowUnavailable,
	#[error("Document unavailable")]
	DocumentUnavailable,
	#[error("Segment {0} not found")]
	SegmentNotFound(usize),
	#[error("Unknown error: {0:?}")]
	Unknown(wasm_bindgen::JsValue),
}

impl From<wasm_bindgen::JsValue> for Error {
	#[inline(always)]
	fn from(value: wasm_bindgen::JsValue) -> Self {
		Self::Unknown(value)
	}
}
