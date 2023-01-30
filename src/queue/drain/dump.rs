use std::{
    sync::mpsc::{self, Sender, SyncSender},
    thread::{self, JoinHandle},
};

lazy_static::lazy_static! {
    static ref DUMP_DEBUG: bool = std::env::var("DUMP_DEBUG").is_ok();
}

/// a garbage heap that spawns another thread and sends filenames to be deleted.
/// the `None` variant is used when no_del is enabled to turn every method into
/// a no op
pub(crate) enum Dump {
    Real {
        /// handle for spawned thread
        handle: JoinHandle<()>,

        /// channel for sending filenames to be deleted
        sender: Sender<String>,

        /// a sync channel for signalling that the thread should exit
        /// immediately
        signal: SyncSender<()>,
    },
    None,
}

#[inline]
fn nil_handler(_file: &String, _e: std::io::Result<()>) {}

#[inline]
fn debug_handler(file: &String, e: std::io::Result<()>) {
    if let Err(e) = e {
        eprintln!("failed to remove {file} with {e}");
    }
}

impl Dump {
    pub(crate) fn new(no_del: bool) -> Self {
        if no_del {
            return Self::None;
        }
        let (sender, receiver) = mpsc::channel();
        let (signal, exit) = mpsc::sync_channel(0);
        const MAX_FILES: usize = 100;
        // check this condition once before the loop
        let err_handler = if *DUMP_DEBUG {
            debug_handler
        } else {
            nil_handler
        };
        let handle = thread::spawn(move || loop {
            if exit.try_recv().is_ok() {
                return;
            }
            // try to receive up to MAX_FILES at once, then par_iter over the
            // received files. loop to continue the process
            let mut files = Vec::new();
            while let Ok(v) = receiver.try_recv() {
                files.push(v);
                if files.len() >= MAX_FILES {
                    break;
                }
            }
            use rayon::prelude::*;
            files.par_iter().for_each(|file| {
                err_handler(file, std::fs::remove_file(&file));
            });
        });

        Self::Real {
            handle,
            sender,
            signal,
        }
    }

    pub(crate) fn send(&self, s: String) {
        match self {
            Dump::Real { sender, .. } => {
                sender.send(s).unwrap();
            }
            Dump::None => {}
        }
    }

    pub(crate) fn shutdown(self) {
        let Self::Real { handle, sender, signal } = self else {
	    return
	};
        time!(e, {
            drop(sender);
        });
        eprintln!(
            "finished dropping after {:.1} s",
            e.as_millis() as f64 / 1000.0
        );
        // it's okay for this to fail because it just means the receiving thread
        // exited first
        let _ = signal.send(());
        drop(signal);
        time!(e, {
            handle.join().unwrap();
        });
        eprintln!(
            "finished dropping after {:.1} s",
            e.as_millis() as f64 / 1000.0
        );
    }
}
