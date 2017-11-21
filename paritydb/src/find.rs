use std::cmp;

use field::iterator::FieldHeaderIterator;
use field::{Error, Header, field_size};
use prefix_tree::OccupiedOffsetIterator;
use record::{ValueSize, Record};

/// Record location.
#[derive(Debug)]
pub enum RecordResult<'a> {
	/// Found existing record
	Found(Record<'a>),
	/// Record does not exist or was deleted.
	NotFound,
	/// Record does no exist in this memory slice, but may in the next one
	OutOfRange,
}

pub fn find_record<'a>(
	data: &'a [u8],
	field_body_size: usize,
	value_size: ValueSize,
	key: &[u8],
) -> Result<RecordResult<'a>, Error> {
	let iter = FieldHeaderIterator::new(data, field_body_size)?;

	let field_size = field_size(field_body_size);
	let mut offset = 0;
	for header in iter {
		let header = header?;
		match header {
			Header::Uninitialized => return Ok(RecordResult::NotFound),
			Header::Inserted => {
				let slice = &data[offset..];
				match Record::extract_key(slice, field_body_size, key.len()).partial_cmp(&key).unwrap() {
					cmp::Ordering::Less => {},
					cmp::Ordering::Equal => {
						let record = Record::new(slice, field_body_size, value_size, key.len());
						return Ok(RecordResult::Found(record));
					},
					cmp::Ordering::Greater => return Ok(RecordResult::NotFound),
				}
			},
			Header::Continued => {},
		}
		offset += field_size;
	}
	Ok(RecordResult::OutOfRange)
}

pub fn iter<'a>(
	data: &'a [u8],
	occupied_offset_iter: OccupiedOffsetIterator<'a>,
	field_body_size: usize,
	key_size: usize,
	value_size: ValueSize
) -> Result<RecordIterator<'a>, Error> {
	let offset = 0;
	let peek_offset = None;
	let field_size = field_size(field_body_size);

	Ok(RecordIterator { data, occupied_offset_iter, offset, peek_offset, field_body_size, field_size, key_size, value_size })
}

pub struct RecordIterator<'a, T = OccupiedOffsetIterator<'a>> {
	data: &'a [u8],
	occupied_offset_iter: T,
	offset: u32,
	peek_offset: Option<u32>,
	field_body_size: usize,
	field_size: usize,
	key_size: usize,
	value_size: ValueSize
}

impl<'a, T: Iterator<Item=u32>> Iterator for RecordIterator<'a, T> {
	type Item = Result<Record<'a>, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		if let None = self.peek_offset {
			let offset = self.offset;
			self.peek_offset = self.occupied_offset_iter.by_ref().skip_while(|i| *i < offset).next();
			self.offset = self.peek_offset.unwrap_or(offset);
		}

		match self.peek_offset {
			Some(offset) => {
				// reached eof
				if offset as usize * self.field_size >= self.data.len() { return None }

				self.offset += 1;

				let slice = &self.data[offset as usize * self.field_size..];

				let header = match Header::from_u8(slice[0]) {
					Ok(header) => header,
					Err(err) => return Some(Err(err)),
				};

				match header {
					Header::Uninitialized => {
						self.peek_offset = None;
						self.next()
					},
					Header::Continued => {
						self.peek_offset = Some(offset + 1);
						self.next()
					},
					Header::Inserted => {
						self.peek_offset = Some(offset + 1);
						let record = Record::new(slice, self.field_body_size, self.value_size, self.key_size);
						Some(Ok(record))
					}
				}
			},
			_ => None
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{find_record, RecordIterator, RecordResult};
	use record;

	fn expect_record(a: RecordResult, key: &[u8], value: &[u8]) {
		if let RecordResult::Found(record) = a {
			let mut k = Vec::new();
			k.resize(key.len(), 0);
			record.read_key(&mut k);
			assert!(record.key_is_equal(key), "Invalid key. Expected: {:?}, got: {:?}", key, k);

			let mut v = Vec::new();
			v.resize(value.len(), 0);
			record.read_value(&mut v);
			assert!(&*v == value, "Invalid value. Expected: {:?}, got: {:?}", value, v);
		} else {
			assert!(false, "Expected to find record, got: {:?}", a);
		}
	}

	fn assert_eq(a: RecordResult, b: RecordResult) {
		match (a, b) {
			(RecordResult::NotFound, RecordResult::NotFound) => return,
			(RecordResult::OutOfRange, RecordResult::OutOfRange) => return,
			(RecordResult::Found(_), RecordResult::Found(_)) => unimplemented!(),
			(a, b) => {
				assert!(false, "Invalid record result. Expected: {:?}, got: {:?}", a, b);
			}
		}
	}

	#[test]
	fn test_find_record() {
		let value_size = record::ValueSize::Constant(0);
		let body_size = 3;
		let data = [1, 1, 2, 3, 1, 4, 5, 6];
		let key = [1, 2, 3];
		let key2 = [4, 5, 6];

		expect_record(find_record(&data, body_size, value_size, &key).unwrap(), &[1, 2, 3], &[]);
		expect_record(find_record(&data, body_size, value_size, &key2).unwrap(), &[4, 5, 6], &[]);
	}

	#[test]
	fn test_find_not_found_record_location_for_reading() {
		let value_size = record::ValueSize::Constant(0);
		let body_size = 3;
		let data = [1, 1, 2, 3, 1, 4, 5, 6];
		let key = [1, 4, 5];
		let location = RecordResult::NotFound;

		assert_eq(location, find_record(&data, body_size, value_size, &key).unwrap());
	}

	#[test]
	fn test_find_out_of_range_record_location_for_reading() {
		let value_size = record::ValueSize::Constant(0);
		let body_size = 3;
		let data = [1, 1, 2, 3, 1, 4, 5, 6];
		let key = [4, 5, 7];
		let location = RecordResult::OutOfRange;

		assert_eq(location, find_record(&data, body_size, value_size, &key).unwrap());
	}

	#[test]
	fn test_find_uninitialized_record_location_for_reading() {
		let value_size = record::ValueSize::Constant(0);
		let body_size = 3;
		let data = [0, 1, 2, 3, 1, 4, 5, 6];
		let key = [1, 2, 3];
		let key2 = [4, 5, 6];
		let location = RecordResult::NotFound;
		let location2 = RecordResult::NotFound;

		assert_eq(location, find_record(&data, body_size, value_size, &key).unwrap());
		assert_eq(location2, find_record(&data, body_size, value_size, &key2).unwrap());
	}

	#[test]
	fn test_iter() {
		let data = &[1, 1, 1, 0, 0, 0, 1, 2, 2, 1, 3, 3, 0, 0, 0, 0, 0, 0, 1, 4, 4, 1, 5, 5];
		let occupied_offset_iter = vec![0u32, 2u32, 3u32, 6u32].into_iter();

		let offset = 0;
		let peek_offset = None;
		let field_body_size = 2;
		let field_size = 3;
		let key_size = 2;
		let value_size = record::ValueSize::Constant(0);

		let records = RecordIterator {
			data,
			occupied_offset_iter,
			offset,
			peek_offset,
			field_body_size,
			field_size,
			key_size,
			value_size,
		};

		let keys: Vec<_> = records.map(|record| {
			let record = record.unwrap();
			let mut v = Vec::with_capacity(key_size);
			v.resize(key_size, 0);
			record.read_key(&mut v);
			v
		}).collect();

		assert_eq!(
			keys,
			vec![
				vec![1, 1],
				vec![2, 2],
				vec![3, 3],
				vec![4, 4],
				vec![5, 5]]);
	}
}
