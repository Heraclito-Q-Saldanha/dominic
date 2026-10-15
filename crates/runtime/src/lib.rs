pub mod dom;
pub mod error;

pub use wasm_bindgen;
pub use wasm_bindgen::prelude::wasm_bindgen as bindgen;

use std::ops;
use std::sync;

#[derive(Debug, Clone)]
pub struct State<T, const CALL_SIZE: usize> {
	value: sync::Arc<sync::RwLock<T>>,
	callbacks: [fn(); CALL_SIZE],
}

pub struct StateRef<'a, T> {
	value: sync::RwLockReadGuard<'a, T>,
}

pub struct StateRefMut<'a, T> {
	callbacks: &'a [fn()],
	value: Option<sync::RwLockWriteGuard<'a, T>>,
}

impl<T, const CALL_SIZE: usize> State<T, CALL_SIZE> {
	#[inline]
	pub fn new(value: T, callbacks: [fn(); CALL_SIZE]) -> Self {
		let value = sync::Arc::new(sync::RwLock::new(value));

		Self { value, callbacks }
	}

	#[inline]
	pub fn get(&self) -> StateRef<'_, T> {
		let value = self.value.read().unwrap();

		StateRef { value }
	}

	#[inline]
	pub fn get_mut(&self) -> StateRefMut<'_, T> {
		let value = Some(self.value.write().unwrap());
		let callbacks = &self.callbacks;

		StateRefMut { value, callbacks }
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
		self.value.as_deref().unwrap()
	}
}

impl<T> ops::DerefMut for StateRefMut<'_, T> {
	#[inline(always)]
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.value.as_deref_mut().unwrap()
	}
}

impl<T> ops::Drop for StateRefMut<'_, T> {
	#[inline(always)]
	fn drop(&mut self) {
		drop(self.value.take());

		for callback in self.callbacks {
			callback();
		}
	}
}
