use field::view::{FieldsView, FieldsViewMut};

/// A view onto database record.
pub struct Record<'a> {
	key: FieldsView<'a>,
	value: FieldsView<'a>,
}

impl<'a> Record<'a> {
	pub fn new(data: &'a [u8], field_body_size: usize, key_size: usize) -> Self {
		let view = FieldsView::new(data, field_body_size);
		let (key, value) = view.split_at(key_size);
		Record {
			key,
			value,
		}
	}

	pub fn read_key(&self, slice: &mut [u8]) {
		self.key.copy_to_slice(slice);
	}

	pub fn key_is_equal(&self, slice: &[u8]) -> bool {
		self.key == slice
	}

	pub fn read_value(&self, slice: &mut [u8]) {
		self.value.copy_to_slice(slice);
	}
}

/// Mutable view onto database record.
pub struct RecordMut<'a> {
	key: FieldsViewMut<'a>,
	value: FieldsViewMut<'a>,
}

impl<'a> RecordMut<'a> {
	pub fn new(data: &'a mut [u8], field_body_size: usize, key_size: usize) -> Self {
		let view = FieldsViewMut::new(data, field_body_size);
		let (key, value) = view.split_at(key_size);
		RecordMut {
			key,
			value,
		}
	}

	pub fn read_key(&self, slice: &mut [u8]) {
		self.key.copy_to_slice(slice);
	}

	pub fn key_is_equal(&self, slice: &[u8]) -> bool {
		self.key == slice
	}

	pub fn read_value(&self, slice: &mut [u8]) {
		self.value.copy_to_slice(slice);
	}

	pub fn write_key(&mut self, slice: &[u8]) {
		self.key.copy_from_slice(slice);
	}

	pub fn write_value(&mut self, slice: &[u8]) {
		self.value.copy_from_slice(slice);
	}
}

#[cfg(test)]
mod tests {
	use super::RecordMut;

	#[test]
	fn test_record_mut_write() {
		let body_size = 15;
		let key_size = 20;
		let mut data = [0u8; 256];
		let key = [0x22; 20];
		let value = [0x33; 220];

		let mut written_key = [0u8; 20];
		let mut written_value = [0u8; 220];
		let mut record = RecordMut::new(&mut data, body_size, key_size);
		record.write_key(&key);
		record.write_value(&value);
		record.read_key(&mut written_key);
		record.read_value(&mut written_value);
		assert_eq!(key, written_key);
		assert_eq!(&value as &[u8], &written_value as &[u8]);
	}
}
