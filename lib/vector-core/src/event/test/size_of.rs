use super::*;
use crate::event::test::common::Name;
use crate::ByteSizeOf;
use quickcheck::{Arbitrary, Gen, QuickCheck, TestResult};
use std::mem;

#[test]
fn at_least_wrapper_size() {
    // The byte size of an `Event` should always be at least as big as the
    // mem::size_of of the `Event`.
    #[allow(clippy::needless_pass_by_value)]
    fn inner(event: Event) -> TestResult {
        let baseline = mem::size_of::<Event>();
        assert!(baseline <= event.size_of());
        TestResult::passed()
    }

    QuickCheck::new()
        .tests(1_000)
        .max_tests(10_000)
        .quickcheck(inner as fn(Event) -> TestResult);
}

#[test]
fn exactly_equal_if_no_allocated_bytes() {
    // The byte size of an `Event` should always be exactly equal to its
    // `mem::size_of` if there are no reported allocated bytes.
    #[allow(clippy::needless_pass_by_value)]
    fn inner(event: Event) -> TestResult {
        let allocated_sz = event.allocated_bytes();
        if allocated_sz == 0 {
            let baseline = mem::size_of::<Event>();
            assert_eq!(baseline, event.size_of());
            return TestResult::passed();
        }
        TestResult::discard()
    }

    QuickCheck::new()
        .tests(1_000)
        .max_tests(10_000)
        .quickcheck(inner as fn(Event) -> TestResult);
}

#[test]
fn size_greater_than_allocated_size() {
    // The total byte size of an `Event` should always be strictly greater than
    // the allocated bytes of the `Event`.
    #[allow(clippy::needless_pass_by_value)]
    fn inner(event: Event) -> TestResult {
        let total_sz = event.size_of();
        let allocated_sz = event.allocated_bytes();

        assert!(total_sz > allocated_sz);
        TestResult::passed()
    }

    QuickCheck::new()
        .tests(1_000)
        .max_tests(10_000)
        .quickcheck(inner as fn(Event) -> TestResult);
}

//
// Log Events
//

/// The action that our model interpreter loop will take.
#[derive(Debug, Clone)]
pub enum Action {
    Contains {
        key: String,
    },
    SizeOf,
    /// Insert a key/value pair into the [`LogEvent`]
    InsertFlat {
        key: String,
        value: Value,
    },
    Remove {
        key: String,
    },
}

impl Arbitrary for Action {
    fn arbitrary(g: &mut Gen) -> Self {
        match u8::arbitrary(g) % 3 {
            0 => Action::InsertFlat {
                key: String::from(Name::arbitrary(g)),
                value: Value::arbitrary(g),
            },
            1 => Action::SizeOf,
            2 => Action::Contains {
                key: String::from(Name::arbitrary(g)),
            },
            3 => Action::Remove {
                key: String::from(Name::arbitrary(g)),
            },
            _ => unreachable!(),
        }
    }
}

#[test]
fn log_operation_maintains_size() {
    // Asserts that the stated size of a LogEvent only changes by the amount
    // that we insert / remove from it and that read-only operations do not
    // change the size.
    fn inner(actions: Vec<Action>, mut log_event: LogEvent) -> TestResult {
        let mut current_size = log_event.size_of();

        for action in actions {
            match action {
                Action::InsertFlat { key, value } => {
                    let new_value_sz = value.size_of();
                    let old_value_sz = log_event.get_flat(&key).map_or(0, |x| x.size_of());
                    if !log_event.contains(&key) {
                        current_size += key.len();
                    }
                    log_event.insert_flat(&key, value);
                    current_size -= old_value_sz;
                    current_size += new_value_sz;
                }
                Action::SizeOf => {
                    assert_eq!(current_size, log_event.size_of());
                }
                Action::Contains { key } => {
                    log_event.contains(key);
                }
                Action::Remove { key } => {
                    let value_sz = log_event.remove(&key).size_of();
                    current_size -= value_sz;
                    current_size -= key.size_of();
                }
            }
        }

        TestResult::passed()
    }

    QuickCheck::new()
        .tests(1_000)
        .max_tests(10_000)
        .quickcheck(inner as fn(Vec<Action>, LogEvent) -> TestResult);
}
