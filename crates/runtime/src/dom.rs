use crate::*;

const SHOW_COMMENT: u32 = 0x80;

pub fn update_attribute(id: usize, name: &str, value: &str) -> error::Result<()> {
	let Some(window) = web_sys::window() else {
		return Err(error::Error::WindowUnavailable);
	};

	let Some(document) = window.document() else {
		return Err(error::Error::DocumentUnavailable);
	};

	let Some(element) = document.query_selector(&format!("[data-dominic-{id}]"))? else {
		return Err(error::Error::AttributeNotFound(id));
	};

	if js_sys::Reflect::has(&element, &name.into())? {
		js_sys::Reflect::set(element.as_ref(), &name.into(), &value.into())?;
	}

	element.set_attribute(name, value)?;

	Ok(())
}

pub fn update_segment(id: usize, value: &str) -> error::Result<()> {
	let Some(window) = web_sys::window() else {
		return Err(error::Error::WindowUnavailable);
	};

	let Some(document) = window.document() else {
		return Err(error::Error::DocumentUnavailable);
	};

	let walker = document.create_tree_walker_with_what_to_show(&document, SHOW_COMMENT)?;

	let opening = format!("dominic:{id}");
	let closing = format!("/dominic:{id}");

	let mut start = None;

	while let Some(marker) = walker.next_node()? {
		let content = marker.node_value();
		if content.as_deref() == Some(opening.as_str()) {
			start = Some(marker);
		} else if content.as_deref() == Some(closing.as_str()) {
			if let Some(start) = &start {
				let range = document.create_range()?;

				range.set_start_after(start)?;
				range.set_end_before(&marker)?;
				range.delete_contents()?;
				range.insert_node(&document.create_text_node(value))?;

				return Ok(());
			}
		}
	}

	Err(error::Error::SegmentNotFound(id))
}
