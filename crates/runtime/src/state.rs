use std::cell;
use std::ops;
use std::rc;

#[derive(Debug, Clone)]
pub struct State<T> {
	value: rc::Rc<cell::RefCell<T>>,
}

pub struct StateRef<'a, T> {
	value: cell::Ref<'a, T>,
}

pub struct StateRefMut<'a, T> {
	value: cell::RefMut<'a, T>,
}

impl<T> State<T> {
	#[inline]
	pub fn new(value: T) -> Self {
		let value = rc::Rc::new(cell::RefCell::new(value));

		Self { value }
	}

	#[inline]
	pub fn get(&self) -> StateRef<'_, T> {
		let value = self.value.borrow();

		StateRef { value }
	}

	#[inline]
	pub fn get_mut(&self) -> StateRefMut<'_, T> {
		let value = self.value.borrow_mut();

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
