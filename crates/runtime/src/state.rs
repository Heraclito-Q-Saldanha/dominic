use std::ops;
use std::sync;

#[derive(Debug, Clone)]
pub struct State<T> {
	value: sync::Arc<sync::RwLock<T>>,
}

pub struct StateRef<'a, T> {
	value: sync::RwLockReadGuard<'a, T>,
}

pub struct StateRefMut<'a, T> {
	value: sync::RwLockWriteGuard<'a, T>,
}

impl<T> State<T> {
	#[inline]
	pub fn new(value: T) -> Self {
		let value = sync::Arc::new(sync::RwLock::new(value));

		Self { value }
	}

	#[inline]
	pub fn get(&self) -> StateRef<'_, T> {
		let value = self.value.read().unwrap();

		StateRef { value }
	}

	#[inline]
	pub fn get_mut(&self) -> StateRefMut<'_, T> {
		let value = self.value.write().unwrap();

		StateRefMut { value }
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
