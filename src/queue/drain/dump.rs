use std::{
    sync::mpsc::{self, Sender, SyncSender},
    thread::{self, JoinHandle},
};

/// a garbage heap that spawns another thread and sends filenames to be
/// deleted.
pub(crate) struct Dump {
    /// handle for spawned thread
    handle: JoinHandle<()>,

    /// channel for sending filenames to be deleted
    sender: Sender<String>,

    /// a sync channel for signalling that the thread should exit
    /// immediately
    signal: SyncSender<()>,
}

impl Dump {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let (signal, exit) = mpsc::sync_channel(0);
        let handle = thread::spawn(move || {
            for file in receiver {
                if exit.try_recv().is_ok() {
                    return;
                }
                let e = std::fs::remove_file(&file);
                if let Err(e) = e {
                    eprintln!("failed to remove {file} with {e}");
                }
            }
        });

        Self {
            handle,
            sender,
            signal,
        }
    }

    pub(crate) fn send(&self, s: String) {
        self.sender.send(s).unwrap();
    }

    pub(crate) fn shutdown(self) {
        time!(e, {
            drop(self.sender);
        });
        eprintln!(
            "finished dropping after {:.1} s",
            e.as_millis() as f64 / 1000.0
        );
        // it's okay for this to fail because it just means the receiving thread
        // exited first
        let _ = self.signal.send(());
        drop(self.signal);
        time!(e, {
            self.handle.join().unwrap();
        });
        eprintln!(
            "finished dropping after {:.1} s",
            e.as_millis() as f64 / 1000.0
        );
    }
}
