//! `picture_saver.rs` saves pictures on a thread of its own, so a bot's network loop never waits
//! for a render: a slow render on a busy machine kept a bot silent past the server's timeout, and
//! the server dropped it. It keeps at most a few pictures waiting and drops the rest.
//!
//! Interface: `PictureSaver` (`Start`, `submit`, `finish`).
//!
//! Example: `let mut saver = PictureSaver::Start(move || Ok(|picture| save(picture)), 2);`
//!
//! Must know:
//! - Gotchas: `Start` makes the save on the saving thread, so a save that may not cross threads (a
//!   Metal renderer) is made there.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;

/// `PictureSaver` is a thread saving pictures of type `T` handed to it, the queue to it, and how
/// many pictures it dropped because the queue was full.
pub struct PictureSaver<T: Send + 'static> {
    sender: SyncSender<T>,
    thread: JoinHandle<Result<usize, String>>,
    dropped_count: usize,
}

impl<T: Send + 'static> PictureSaver<T> {
    /// `Start` starts the saving thread: it makes its save with `make_save`, then saves each
    /// picture handed to it in turn, with at most `capacity` waiting.
    ///
    /// Example: `let mut saver = PictureSaver::Start(move || Ok(|picture| save(picture)), 2);`
    #[allow(non_snake_case)]
    pub fn Start<S, M>(make_save: M, capacity: usize) -> PictureSaver<T>
    where
        M: FnOnce() -> Result<S, String> + Send + 'static,
        S: FnMut(T) -> Result<(), String>,
    {
        let (sender, receiver): (SyncSender<T>, Receiver<T>) = mpsc::sync_channel(capacity);
        let thread: JoinHandle<Result<usize, String>> = std::thread::spawn(move || {
            let mut save: S = make_save()?;
            let mut saved_count: usize = 0;
            for picture in receiver {
                save(picture)?;
                saved_count += 1;
            }
            Ok(saved_count)
        });
        PictureSaver {
            sender,
            thread,
            dropped_count: 0,
        }
    }

    /// `submit` hands `picture` to the saving thread without waiting, and says whether it took it:
    /// it drops the picture when the queue is full, or when the thread stopped on an error (which
    /// `finish` gives).
    ///
    /// Example: `saver.submit(picture);`
    pub fn submit(&mut self, picture: T) -> bool {
        match self.sender.try_send(picture) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped_count += 1;
                false
            }
        }
    }

    /// `finish` waits until the pictures waiting are saved and gives how many it saved and dropped,
    /// or the error that stopped the saving thread.
    ///
    /// Example: `let (saved_count, dropped_count) = saver.finish()?;`
    pub fn finish(self) -> Result<(usize, usize), String> {
        drop(self.sender);
        let saved_count: usize = self
            .thread
            .join()
            .map_err(|_| "the picture saving thread panicked".to_string())??;
        Ok((saved_count, self.dropped_count))
    }
}

#[cfg(test)]
mod tests {
    //! Tests of saving pictures on their own thread.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Save speed | slower than the pictures come (queue fills, pictures dropped, `submit` never waits) | `submitting_never_waits_for_a_slow_save` |
    //! | Save result | every save works, a save fails (the error comes out of `finish`) | `submitting_never_waits_for_a_slow_save`, `a_failed_save_comes_out_of_finish` |

    use std::time::{Duration, Instant};

    use super::*;

    const SLOW_SAVE_MS: u64 = 300;
    const MAX_SUBMIT_MS: u128 = 100;
    const PICTURE_COUNT: usize = 4;

    #[test]
    fn submitting_never_waits_for_a_slow_save() {
        let save = |_: usize| -> Result<(), String> {
            std::thread::sleep(Duration::from_millis(SLOW_SAVE_MS));
            Ok(())
        };
        let mut saver: PictureSaver<usize> = PictureSaver::Start(move || Ok(save), 1);
        let started: Instant = Instant::now();
        for picture in 0..PICTURE_COUNT {
            saver.submit(picture);
        }
        assert!(started.elapsed().as_millis() < MAX_SUBMIT_MS);
        let (saved_count, dropped_count) = saver.finish().expect("every save works");
        assert!(saved_count >= 1);
        assert_eq!(saved_count + dropped_count, PICTURE_COUNT);
    }

    #[test]
    fn a_failed_save_comes_out_of_finish() {
        let save = |_: usize| -> Result<(), String> { Err("disk full".to_string()) };
        let mut saver: PictureSaver<usize> = PictureSaver::Start(move || Ok(save), 1);
        saver.submit(0);
        assert_eq!(saver.finish(), Err("disk full".to_string()));
    }
}
