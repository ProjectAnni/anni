// This file is a part of simple_audio
// Copyright (c) 2022-2023 Erikas Taroza <erikastaroza@gmail.com>

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    thread,
};

use crossbeam::queue::ArrayQueue;

#[derive(Clone)]
pub struct Producer;

#[derive(Clone)]
pub struct Consumer;

struct Inner<T> {
    queue: ArrayQueue<T>,
    cancelled: AtomicBool,
    producer_thread: OnceLock<thread::Thread>,
}

/// A bounded queue with a blocking producer and a lock-free realtime consumer.
///
/// There is one producer (the decoder) and one consumer (the CPAL callback). The
/// consumer never waits and never takes a mutex.
pub struct BlockingRb<T, Type = Producer> {
    inner: Arc<Inner<T>>,
    _type: std::marker::PhantomData<Type>,
}

impl<T, Type> Clone for BlockingRb<T, Type> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _type: std::marker::PhantomData,
        }
    }
}

impl<T, Type> BlockingRb<T, Type> {
    pub fn new(size: usize) -> (BlockingRb<T, Producer>, BlockingRb<T, Consumer>) {
        assert!(size > 0, "ring buffer capacity must be greater than zero");

        let inner = Arc::new(Inner {
            queue: ArrayQueue::new(size),
            cancelled: AtomicBool::new(false),
            producer_thread: OnceLock::new(),
        });

        (
            BlockingRb {
                inner: Arc::clone(&inner),
                _type: std::marker::PhantomData,
            },
            BlockingRb {
                inner,
                _type: std::marker::PhantomData,
            },
        )
    }

    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.queue.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.queue.is_empty()
    }
}

impl<T: Copy> BlockingRb<T, Producer> {
    /// Writes as many items as currently fit, waiting when the queue is full.
    /// Returns `None` only for an empty input or after cancellation.
    pub fn write(&self, slice: &[T]) -> Option<usize> {
        if slice.is_empty() {
            return None;
        }
        let current_thread = thread::current();
        let producer_thread = self
            .inner
            .producer_thread
            .get_or_init(|| current_thread.clone());
        assert_eq!(
            producer_thread.id(),
            current_thread.id(),
            "BlockingRb supports exactly one producer thread"
        );

        loop {
            if self.inner.cancelled.load(Ordering::Acquire) {
                return None;
            }

            let mut written = 0;
            for &value in slice {
                if self.inner.queue.push(value).is_err() {
                    break;
                }
                written += 1;
            }

            if written > 0 {
                return Some(written);
            }

            thread::park();
        }
    }

    /// Permanently cancels this producer and wakes a blocked write.
    ///
    /// Cancellation is one-way: this producer will return `None` from every
    /// subsequent [`Self::write`] call. Create a new ring buffer to resume
    /// writing after cancellation.
    pub fn cancel_write(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        if let Some(producer) = self.inner.producer_thread.get() {
            producer.unpark();
        }
    }
}

impl<T: Copy> BlockingRb<T, Consumer> {
    pub fn drain_with(&self, limit: usize, mut consume: impl FnMut(usize, T)) -> usize {
        let mut read = 0;
        while read < limit {
            let Some(value) = self.inner.queue.pop() else {
                break;
            };
            consume(read, value);
            read += 1;
        }

        if read > 0
            && let Some(producer) = self.inner.producer_thread.get()
        {
            producer.unpark();
        }
        read
    }

    /// Reads only values that were actually written. Any unfilled portion of
    /// `slice` is left untouched for the caller to fill with silence.
    #[cfg(test)]
    pub fn read(&self, slice: &mut [T]) -> Option<usize> {
        if slice.is_empty() {
            return None;
        }

        let limit = slice.len();
        let read = self.drain_with(limit, |index, value| slice[index] = value);
        (read > 0).then_some(read)
    }

    pub fn skip_all(&self) -> usize {
        let mut skipped = 0;
        while self.inner.queue.pop().is_some() {
            skipped += 1;
        }
        if skipped > 0
            && let Some(producer) = self.inner.producer_thread.get()
        {
            producer.unpark();
        }
        skipped
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use super::BlockingRb;

    #[test]
    fn partial_read_does_not_consume_unwritten_values() {
        let (writer, reader) = BlockingRb::<u32>::new(8);
        assert_eq!(writer.write(&[11, 22]), Some(2));

        let mut output = [99; 4];
        assert_eq!(reader.read(&mut output), Some(2));
        assert_eq!(output, [11, 22, 99, 99]);
        assert!(reader.is_empty());
    }

    #[test]
    fn wraps_without_reordering_samples() {
        let (writer, reader) = BlockingRb::<u32>::new(4);
        assert_eq!(writer.write(&[1, 2, 3, 4]), Some(4));

        let mut first = [0; 3];
        assert_eq!(reader.read(&mut first), Some(3));
        assert_eq!(first, [1, 2, 3]);

        assert_eq!(writer.write(&[5, 6, 7]), Some(3));
        let mut second = [0; 4];
        assert_eq!(reader.read(&mut second), Some(4));
        assert_eq!(second, [4, 5, 6, 7]);
    }

    #[test]
    fn skip_all_reports_discarded_values() {
        let (writer, reader) = BlockingRb::<u32>::new(4);
        assert_eq!(writer.write(&[1, 2, 3]), Some(3));
        assert_eq!(reader.skip_all(), 3);
        assert!(reader.is_empty());
    }

    #[test]
    fn blocked_writer_wakes_when_the_consumer_drains() {
        let (writer, reader) = BlockingRb::<u32>::new(1);
        let (ready_sender, ready_receiver) = mpsc::channel();
        let (done_sender, done_receiver) = mpsc::channel();
        let producer = thread::spawn(move || {
            assert_eq!(writer.write(&[1]), Some(1));
            ready_sender.send(()).unwrap();
            done_sender.send(writer.write(&[2])).unwrap();
        });

        ready_receiver.recv().unwrap();
        assert!(done_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_err());
        let mut first = [0];
        assert_eq!(reader.read(&mut first), Some(1));
        assert_eq!(first, [1]);
        assert_eq!(
            done_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            Some(1)
        );
        producer.join().unwrap();

        let mut second = [0];
        assert_eq!(reader.read(&mut second), Some(1));
        assert_eq!(second, [2]);
    }

    #[test]
    fn cancellation_unparks_a_blocked_writer() {
        let (writer, _reader) = BlockingRb::<u32>::new(1);
        let cancel = writer.clone();
        let (ready_sender, ready_receiver) = mpsc::channel();
        let (done_sender, done_receiver) = mpsc::channel();
        let producer = thread::spawn(move || {
            assert_eq!(writer.write(&[1]), Some(1));
            ready_sender.send(()).unwrap();
            done_sender.send(writer.write(&[2])).unwrap();
        });

        ready_receiver.recv().unwrap();
        assert!(done_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_err());
        cancel.cancel_write();
        assert_eq!(
            done_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            None
        );
        producer.join().unwrap();
    }
}
