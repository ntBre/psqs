use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

use crate::program::dftbplus::DFTBPlus;
use crate::program::molpro::Molpro;
use crate::program::{mopac::Mopac, Program};
use crate::queue::Queue;

use super::{SubQueue, Submit};

/// Minimal implementation for testing MOPAC locally
#[derive(Debug)]
pub struct Local {
    pub dir: String,
    pub chunk_size: usize,
    pub mopac: String,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            dir: ".".to_string(),
            chunk_size: 128,
            mopac: "/opt/mopac/mopac".to_owned(),
        }
    }
}

impl Local {
    pub fn new(
        chunk_size: usize,
        _job_limit: usize,
        _sleep_int: usize,
        dir: &'static str,
        _no_del: bool,
        _template: Option<String>,
    ) -> Self {
        Self {
            dir: dir.to_string(),
            chunk_size,
            mopac: "/opt/mopac/mopac".to_string(),
        }
    }
}

impl Submit<Molpro> for Local {}

impl Queue<Molpro> for Local {
    fn default_submit_script(&self) -> String {
        todo!()
    }

    fn write_submit_script(&self, _infiles: &[String], _filename: &str) {
        todo!()
    }
}

impl Submit<Mopac> for Local {}

impl Queue<Mopac> for Local {
    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        use std::fmt::Write;
        let mut body = String::from("export LD_LIBRARY_PATH=/opt/mopac/\n");
        for f in infiles {
            writeln!(body, "{} {f}.mop &> {filename}.out", self.mopac).unwrap();
            writeln!(body, "cat {f}.mop {f}.out >> {filename}.out").unwrap();
            writeln!(body, "echo \"================\" >> {filename}.out")
                .unwrap();
        }
        writeln!(body, "date +%s >> {filename}.out").unwrap();
        let mut file = File::create(filename).unwrap_or_else(|_| {
            panic!("failed to create submit script `{filename}`")
        });
        write!(file, "{body}").expect("failed to write submit script");
    }

    fn default_submit_script(&self) -> String {
        todo!()
    }
}

impl Submit<DFTBPlus> for Local {}

impl Queue<DFTBPlus> for Local {
    fn default_submit_script(&self) -> String {
        todo!()
    }

    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        use std::fmt::Write;
        let mut body = String::new();
        // assume f is a directory name, not a real file
        let c = std::env::var("DFTB_PATH").unwrap_or("/opt/dftb+/dftb+".into());
        for f in infiles {
            writeln!(body, "(cd {f} && {c} > out)").unwrap();
        }
        writeln!(body, "date +%s >> {filename}.out").unwrap();
        let mut file = File::create(filename).unwrap_or_else(|_| {
            panic!("failed to create submit script `{filename}`")
        });
        write!(file, "{body}").expect("failed to write submit script");
    }
}

impl<P: Program + Clone + Serialize + for<'a> Deserialize<'a>> SubQueue<P>
    for Local
{
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

    fn status(&self) -> HashSet<String> {
        for dir in ["opt", "pts", "freqs"] {
            let d = std::fs::read_dir(dir).unwrap();
            for f in d {
                eprintln!("contents of {:?}", f.as_ref().unwrap());
                eprintln!(
                    "{}",
                    std::fs::read_to_string(f.unwrap().path()).unwrap()
                );
                eprintln!("================");
            }
        }
        panic!("no status available for Local queue");
    }

    fn no_del(&self) -> bool {
        false
    }
}
