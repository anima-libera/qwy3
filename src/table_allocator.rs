/// Manages the knowledge of which entry is allocated or free in a table,
/// in a way that grantees an amortized O(1) allocation, at the cost of worst case O(n) freeing.
/// The freeing won't be that bad in practice if the memory is not fragmented in a
/// severely unlucky way.
pub(crate) struct TableAllocator {
	length: usize,
	/// The intervals of consecutive free entries by their indices.
	/// If an index is in one of these intervals, then it corresponds to a free entry,
	/// else it correspond to an allocated entry.
	///
	/// Invariants:
	///   - All the intervals are valid and non-empty.
	///   - Always sorted.
	///   - No touching intervals, they would have been merged.
	///   - No overlapping intervals.
	free_intervals: Vec<FreeInterval>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FreeInterval {
	inf: usize,
	sup_excluded: usize,
}

impl PartialOrd for FreeInterval {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		if self.sup_excluded <= other.inf {
			Some(std::cmp::Ordering::Less)
		} else if self == other {
			Some(std::cmp::Ordering::Equal)
		} else if self.inf >= other.sup_excluded {
			Some(std::cmp::Ordering::Greater)
		} else {
			None
		}
	}
}

impl FreeInterval {
	fn length(self) -> usize {
		self.sup_excluded - self.inf
	}

	fn contains(self, index: usize) -> bool {
		(self.inf..self.sup_excluded).contains(&index)
	}
}

pub(crate) enum AllocationDecision {
	/// The allocator decided to allocate the given index.
	AllocateIndex(usize),
	/// There is no more free slot, the allocator needs to be lengthened.
	NeedsBiggerBuffer,
}

impl TableAllocator {
	pub(crate) fn new(length: usize) -> TableAllocator {
		let mut table_allocator = TableAllocator { length: 0, free_intervals: vec![] };
		if 0 < length {
			table_allocator.length_increased_to(length);
		}
		table_allocator
	}

	/// Let the allocator pick a free index that becomes allocated.
	/// It may return `NeedsBiggerBuffer` in case it runs out of space,
	/// and if such lengthening happens, it must be communicated to the allocator
	/// via the `length_increased_to` method.
	pub(crate) fn allocate_one(&mut self) -> AllocationDecision {
		let first_interval = match self.free_intervals.last() {
			Some(interval) => interval,
			None => return AllocationDecision::NeedsBiggerBuffer,
		};
		let decision = AllocationDecision::AllocateIndex(first_interval.sup_excluded - 1);
		self.free_intervals.last_mut().unwrap().sup_excluded -= 1;
		if self.free_intervals.last().unwrap().length() == 0 {
			self.free_intervals.pop();
		}
		decision
	}

	/// Identifies the interval or the space between intervals where the given index lands.
	fn where_index_lands(&self, index: usize) -> WhereIndexLands {
		assert!(index < self.length);
		let mut search_inf = 0;
		let mut search_sup_excluded = self.free_intervals.len();
		loop {
			let seach_middle = (search_inf + search_sup_excluded) / 2;
			let interval = self.free_intervals[seach_middle];

			if interval.contains(index) {
				// We found an interval that contains the index.
				return WhereIndexLands::InInterval(seach_middle);
			} else if index < interval.inf {
				// The index is before the interval, is it between `interval` and the interval before?
				if let Some(interval_before) = self.free_intervals.get(seach_middle - 1) {
					if interval_before.sup_excluded <= index {
						return WhereIndexLands::BeforeInterval(seach_middle);
					}
				} else {
					return WhereIndexLands::BeforeInterval(seach_middle);
				}
				// No, the seach goes on.
				search_sup_excluded = seach_middle;
			} else if interval.sup_excluded <= index {
				// The index is after the interval, is it between `interval` and the interval after?
				if let Some(interval_after) = self.free_intervals.get(seach_middle + 1) {
					if index < interval_after.inf {
						return WhereIndexLands::BeforeInterval(seach_middle + 1);
					}
				} else {
					return WhereIndexLands::BeforeInterval(seach_middle + 1);
				}
				// No, the seach goes on.
				search_inf = seach_middle + 1;
			} else {
				unreachable!();
			}

			if search_inf == search_sup_excluded {
				panic!("Bug: We missed it? How?");
			}
		}
	}

	/// Frees the given index (that must have been allocated before).
	pub(crate) fn free_one(&mut self, index: usize) {
		match self.where_index_lands(index) {
			WhereIndexLands::InInterval(_interval_i) => panic!("Double free"),
			WhereIndexLands::BeforeInterval(interval_after_i) => {
				let interval_before = (interval_after_i >= 1)
					.then(|| self.free_intervals.get(interval_after_i - 1).unwrap());
				let interval_after = self.free_intervals.get(interval_after_i);

				let extends_before =
					interval_before.is_some_and(|interval_before| interval_before.sup_excluded == index);
				let extends_after =
					interval_after.is_some_and(|interval_after| interval_after.inf == index);

				if extends_before && extends_after {
					// The now free index was just what was needed to merge the surrounding intervals.
					self.free_intervals[interval_after_i - 1] = FreeInterval {
						inf: interval_before.unwrap().inf,
						sup_excluded: interval_after.unwrap().sup_excluded,
					};
					self.free_intervals.remove(interval_after_i);
				} else if extends_before {
					// The index extends the interval before.
					self.free_intervals[interval_after_i - 1].sup_excluded += 1;
				} else if extends_after {
					// The index extends the interval after.
					self.free_intervals[interval_after_i].inf -= 1;
				} else {
					// The index is isolated from the surrounding intervals
					// and will form a new interval of its own.
					self.free_intervals.insert(
						interval_after_i,
						FreeInterval { inf: index, sup_excluded: index + 1 },
					)
				}
			},
		}
	}

	/// Communicates to the allocator that it now has a new bigger length.
	/// The added space is considered free.
	pub(crate) fn length_increased_to(&mut self, new_length: usize) {
		assert!(self.length < new_length);
		if self
			.free_intervals
			.last()
			.is_some_and(|last_interval| last_interval.sup_excluded == self.length)
		{
			// The last interval touched the end at the previous length,
			// so we just extend the last interval to cover the new portion.
			self.free_intervals.last_mut().unwrap().sup_excluded = new_length;
		} else {
			// The new portion is isolated from the previous last interval and will form
			// and interval of its own.
			self.free_intervals.push(FreeInterval { inf: self.length, sup_excluded: new_length });
		}
		self.length = new_length;
	}
}

enum WhereIndexLands {
	/// The index lands inside the interval `free_intervals[i]`.
	InInterval(usize),
	/// The index lands before the interval `free_intervals[i]`,
	/// but is after the interval before that (if any).
	/// The given `i` may be 1 after the last interval index, in which case it indicates
	/// that the index lands after the last interval.
	BeforeInterval(usize),
}
