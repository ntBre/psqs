use std::fs::File;
use std::io::Write;

use crate::program::Program;
use crate::queue::Queue;

/// Minimal implementation for testing MOPAC locally
#[derive(Debug)]
pub struct LocalQueue {
    pub dir: String,
    pub chunk_size: usize,
}

impl Default for LocalQueue {
    fn default() -> Self {
        Self {
            dir: ".".to_string(),
            chunk_size: 128,
        }
    }
}

impl LocalQueue {
    pub fn new(dir: &str, chunk_size: usize) -> Self {
        Self {
            dir: dir.to_string(),
            chunk_size,
        }
    }
}

impl<P: Program + Clone> Queue<P> for LocalQueue {
    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        let mut body = String::from("export LD_LIBRARY_PATH=/opt/mopac/\n");
        for f in infiles {
            body.push_str(&format!("/opt/mopac/mopac {f}.mop\n"));
        }
        body.push_str(&format!("date +%s\n"));
        let mut file = File::create(filename)
            .expect(&format!("failed to create submit script `{filename}`"));
        write!(file, "{}", body).expect("failed to write submit script");
    }

    fn submit_command(&self) -> &str {
        "bash"
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    fn job_limit(&self) -> usize {
        1600
    }

    fn sleep_int(&self) -> usize {
        1
    }

    const SCRIPT_EXT: &'static str = "slurm";

    fn dir(&self) -> &str {
        &self.dir
    }

    fn stat_cmd(&self) -> String {
        todo!()
    }
}
