use alloc::{collections::VecDeque, vec::Vec};

pub const PRESENTATION_CURSOR_BEGIN: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationCursors {
    pub snapshots: u64,
    pub events: u64,
    pub timeline: u64,
    pub action_receipts: u64,
    pub release_samples: u64,
}

impl Default for PresentationCursors {
    fn default() -> Self {
        Self {
            snapshots: PRESENTATION_CURSOR_BEGIN,
            events: PRESENTATION_CURSOR_BEGIN,
            timeline: PRESENTATION_CURSOR_BEGIN,
            action_receipts: PRESENTATION_CURSOR_BEGIN,
            release_samples: PRESENTATION_CURSOR_BEGIN,
        }
    }
}

impl PresentationCursors {
    pub const fn validate(self) -> Result<(), CursorError> {
        if self.snapshots == 0
            || self.events == 0
            || self.timeline == 0
            || self.action_receipts == 0
            || self.release_samples == 0
        {
            return Err(CursorError::Zero);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequenced<T> {
    pub sequence: u64,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationBatch<T> {
    pub requested_cursor: u64,
    pub next_cursor: u64,
    pub oldest_available: u64,
    pub newest_available: u64,
    pub records: Vec<Sequenced<T>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorError {
    Zero,
    Capacity,
    Limit,
    Overflow,
    ResyncRequired { oldest_available: u64 },
    Ahead { next_available: u64 },
}

#[derive(Clone, Debug)]
pub struct RetainedStream<T> {
    capacity: usize,
    oldest_sequence: u64,
    next_sequence: u64,
    records: VecDeque<T>,
}

impl<T> RetainedStream<T> {
    pub fn new(capacity: usize) -> Result<Self, CursorError> {
        if capacity == 0 {
            return Err(CursorError::Capacity);
        }
        Ok(Self {
            capacity,
            oldest_sequence: PRESENTATION_CURSOR_BEGIN,
            next_sequence: PRESENTATION_CURSOR_BEGIN,
            records: VecDeque::with_capacity(capacity),
        })
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn oldest_cursor(&self) -> u64 {
        self.oldest_sequence
    }

    pub const fn next_cursor(&self) -> u64 {
        self.next_sequence
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn push(&mut self, value: T) -> Result<u64, CursorError> {
        if self.next_sequence == u64::MAX {
            return Err(CursorError::Overflow);
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        if self.records.len() == self.capacity {
            self.records.pop_front();
            self.oldest_sequence += 1;
        }
        self.records.push_back(value);
        Ok(sequence)
    }
}

impl<T: Clone> RetainedStream<T> {
    pub fn read(
        &self,
        cursor: u64,
        max_records: usize,
    ) -> Result<PresentationBatch<T>, CursorError> {
        if cursor == 0 {
            return Err(CursorError::Zero);
        }
        if max_records == 0 {
            return Err(CursorError::Limit);
        }
        if cursor < self.oldest_sequence {
            return Err(CursorError::ResyncRequired {
                oldest_available: self.oldest_sequence,
            });
        }
        if cursor > self.next_sequence {
            return Err(CursorError::Ahead {
                next_available: self.next_sequence,
            });
        }

        let offset =
            usize::try_from(cursor - self.oldest_sequence).map_err(|_| CursorError::Overflow)?;
        let mut records = Vec::with_capacity(max_records.min(self.records.len()));
        for (index, value) in self
            .records
            .iter()
            .skip(offset)
            .take(max_records)
            .enumerate()
        {
            let sequence = cursor
                .checked_add(index as u64)
                .ok_or(CursorError::Overflow)?;
            records.push(Sequenced {
                sequence,
                value: value.clone(),
            });
        }
        let next_cursor = cursor
            .checked_add(records.len() as u64)
            .ok_or(CursorError::Overflow)?;
        Ok(PresentationBatch {
            requested_cursor: cursor,
            next_cursor,
            oldest_available: self.oldest_sequence,
            newest_available: self.next_sequence.saturating_sub(1),
            records,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CoalescingSnapshot<T> {
    next_sequence: u64,
    latest: Option<Sequenced<T>>,
}

impl<T> Default for CoalescingSnapshot<T> {
    fn default() -> Self {
        Self {
            next_sequence: PRESENTATION_CURSOR_BEGIN,
            latest: None,
        }
    }
}

impl<T> CoalescingSnapshot<T> {
    pub fn publish(&mut self, value: T) -> Result<u64, CursorError> {
        if self.next_sequence == u64::MAX {
            return Err(CursorError::Overflow);
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.latest = Some(Sequenced { sequence, value });
        Ok(sequence)
    }

    pub fn latest(&self) -> Option<&Sequenced<T>> {
        self.latest.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_cursors_start_at_first_sequence() {
        assert_eq!(
            PresentationCursors::default(),
            PresentationCursors {
                snapshots: 1,
                events: 1,
                timeline: 1,
                action_receipts: 1,
                release_samples: 1,
            }
        );
    }

    #[test]
    fn retained_stream_reports_gaps_instead_of_hiding_them() {
        let mut stream = RetainedStream::new(2).unwrap();
        assert_eq!(stream.push(10), Ok(1));
        assert_eq!(stream.push(20), Ok(2));
        assert_eq!(stream.push(30), Ok(3));
        assert_eq!(
            stream.read(1, 8),
            Err(CursorError::ResyncRequired {
                oldest_available: 2
            })
        );
        let batch = stream.read(2, 8).unwrap();
        assert_eq!(
            batch.records,
            alloc::vec![
                Sequenced {
                    sequence: 2,
                    value: 20
                },
                Sequenced {
                    sequence: 3,
                    value: 30
                }
            ]
        );
        assert_eq!(batch.next_cursor, 4);
    }

    #[test]
    fn cursor_ahead_and_empty_tail_are_distinct() {
        let mut stream = RetainedStream::new(2).unwrap();
        stream.push(10).unwrap();
        assert!(stream.read(2, 8).unwrap().records.is_empty());
        assert_eq!(
            stream.read(3, 8),
            Err(CursorError::Ahead { next_available: 2 })
        );
    }

    #[test]
    fn snapshots_coalesce_but_keep_publication_sequence() {
        let mut slot = CoalescingSnapshot::default();
        assert_eq!(slot.publish("first"), Ok(1));
        assert_eq!(slot.publish("latest"), Ok(2));
        assert_eq!(
            slot.latest(),
            Some(&Sequenced {
                sequence: 2,
                value: "latest"
            })
        );
    }
}
