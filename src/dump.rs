pub(crate) struct Dump {
    pub(crate) buf: Vec<String>,
    pub(crate) ptr: usize,
    pub(crate) max: usize,
}

use std::sync::Once;
static mut DEBUG: bool = false;
static INIT: Once = Once::new();

/// check the `SEMP_DUMP_DEBUG` environment variable on first call and from then
/// on report whether or not it was set to `1`
fn is_debug() -> bool {
    unsafe {
        INIT.call_once(|| {
            let v = std::env::var("SEMP_DUMP_DEBUG").unwrap_or_default();
            if v == "1" {
                DEBUG = true;
            }
        });
        DEBUG
    }
}

impl Dump {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            buf: vec![String::new(); size],
            ptr: 0,
            max: size,
        }
    }

    pub(crate) fn add(&mut self, files: Vec<String>) {
        for file in files {
            if self.ptr == self.max - 1 {
                self.dump();
            }
            self.buf[self.ptr] = file;
            self.ptr += 1;
        }
    }

    fn dump(&mut self) {
        let now = std::time::Instant::now();
        let n = self.buf.len();
        for file in &self.buf {
            let _ = std::fs::remove_file(file);
        }
        if is_debug() {
            eprintln!(
                "finished dumping {n} jobs after {:.1} sec",
                now.elapsed().as_millis() as f64 / 1e3
            );
        }
        self.ptr = 0;
    }
}
