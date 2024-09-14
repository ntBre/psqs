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
    pub template: Option<String>,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            dir: ".".to_string(),
            chunk_size: 128,
            mopac: "/opt/mopac/mopac".to_owned(),
            template: None,
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
        template: Option<String>,
    ) -> Self {
        Self {
            dir: dir.to_string(),
            chunk_size,
            mopac: "/opt/mopac/mopac".to_string(),
            template,
        }
    }
}

impl Submit<Molpro> for Local {}

impl Queue<Molpro> for Local {
    fn default_submit_script(&self) -> String {
        todo!()
    }

    fn write_submit_script(
        &self,
        _infiles: impl IntoIterator<Item = String>,
        _filename: &str,
    ) {
        todo!()
    }
}

impl Submit<Mopac> for Local {}

impl Queue<Mopac> for Local {
    fn write_submit_script(
        &self,
        infiles: impl IntoIterator<Item = String>,
        filename: &str,
    ) {
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

    fn write_submit_script(
        &self,
        infiles: impl IntoIterator<Item = String>,
        filename: &str,
    ) {
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
            let Ok(d) = std::fs::read_dir(dir) else {
                log::error!("{dir} not found for status");
                continue;
            };
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

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::program::cfour::Cfour;

    use super::*;

    fn local() -> Local {
        Local {
            dir: String::new(),
            chunk_size: 0,
            mopac: "mopac".into(),
            template: None,
        }
    }

    macro_rules! make_tests {
        ($($name:ident, $queue:expr => $p:ty$(,)*)*) => {
            $(
            #[test]
            fn $name() {
                let tmp = tempfile::NamedTempFile::new().unwrap();
                <Local as Queue<$p>>::write_submit_script(
                    $queue,
                    ["opt0.inp", "opt1.inp", "opt2.inp", "opt3.inp"].map(|s| s.into()),
                    tmp.path().to_str().unwrap(),
                );
                let got = std::fs::read_to_string(tmp).unwrap();
                let got: Vec<&str> = got.lines().filter(|l|
                    !l.contains("/tmp")).collect();
                let got = got.join("\n");
                assert_snapshot!(got);
            }
            )*
        }
    }

    make_tests! {
        mopac_local, &local() =>  Mopac,
        // molpro_local, &local() =>  Molpro,
        cfour_local, &local() => Cfour,
        dftb_local, &local() => DFTBPlus,
    }
}
