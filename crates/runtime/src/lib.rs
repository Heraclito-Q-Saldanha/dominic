pub use wasm_bindgen;
pub use wasm_bindgen::prelude::wasm_bindgen as bindgen;

use std::ops;
use std::sync;

static ID_COUNTER: sync::atomic::AtomicUsize = sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Clone)]
pub struct State<T> {
	id: usize,
	value: sync::Arc<sync::RwLock<T>>,
}

pub struct StateRef<'a, T> {
	value: sync::RwLockReadGuard<'a, T>,
}

pub struct StateRefMut<'a, T> {
	id: usize,
	value: sync::RwLockWriteGuard<'a, T>,
}

impl<T> State<T> {
	#[inline]
	pub fn new(value: T) -> Self {
		let value = sync::Arc::new(sync::RwLock::new(value));
		let id = ID_COUNTER.fetch_add(1, sync::atomic::Ordering::Relaxed);

		Self { id, value }
	}

	#[inline]
	pub fn get(&self) -> StateRef<'_, T> {
		let value = self.value.read().unwrap();

		StateRef { value }
	}

	#[inline]
	pub fn get_mut(&self) -> StateRefMut<'_, T> {
		let id = self.id;
		let value = self.value.write().unwrap();

		StateRefMut { id, value }
	}
}

impl<T> ops::Deref for StateRef<'_, T> {
	type Target = T;

	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		self.value.deref()
	}
}

impl<T> ops::Deref for StateRefMut<'_, T> {
	type Target = T;

	#[inline(always)]
	fn deref(&self) -> &Self::Target {
		self.value.deref()
	}
}

impl<T> ops::DerefMut for StateRefMut<'_, T> {
	#[inline(always)]
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.value.deref_mut()
	}
}

impl<T> ops::Drop for StateRefMut<'_, T> {
	#[inline(always)]
	fn drop(&mut self) {
		let _ = format!("sync dom id: {} dropped", self.id);
	}
}
