use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::program::mopac::Mopac;
use crate::program::Program;
use crate::program::{dftbplus::DFTBPlus, molpro::Molpro};
use crate::queue::Queue;

use super::{SubQueue, Submit};

/// Slurm is a type for holding the information for submitting a slurm job.
/// `filename` is the name of the Slurm submission script
#[derive(Debug)]
pub struct Slurm {
    chunk_size: usize,
    job_limit: usize,
    sleep_int: usize,
    dir: &'static str,
    no_del: bool,
    template: Option<String>,
}

impl Slurm {
    pub fn new(
        chunk_size: usize,
        job_limit: usize,
        sleep_int: usize,
        dir: &'static str,
        no_del: bool,
        template: Option<String>,
    ) -> Self {
        Self {
            chunk_size,
            job_limit,
            sleep_int,
            dir,
            no_del,
            template,
        }
    }
}

impl<P: Program + Clone + Serialize + for<'a> Deserialize<'a>> Submit<P>
    for Slurm
{
}

impl Queue<Molpro> for Slurm {
    fn write_submit_script(
        &self,
        infiles: impl IntoIterator<Item = String>,
        filename: &str,
    ) {
        let mut body = self
            .template
            .clone()
            .unwrap_or_else(|| {
                <Self as Queue<Molpro>>::default_submit_script(self)
            })
            .replace("{{.filename}}", filename);
        for f in infiles {
            body.push_str(&format!("$MOLPRO_CMD {f}.inp\n"));
        }
        let mut file = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("write_submit_script: failed to create {filename}");
                std::process::exit(1);
            }
        };
        write!(file, "{body}").unwrap_or_else(|_| {
            panic!("failed to write molpro input file: {filename}")
        });
    }

    fn default_submit_script(&self) -> String {
        "#!/bin/bash
#SBATCH --job-name={{.filename}}
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=1
#SBATCH -o {{.filename}}.out
#SBATCH --no-requeue
#SBATCH --mem=8gb

MOLPRO_CMD=\"/home/qc/bin/molpro2020.sh 1 1\"
"
        .to_owned()
    }
}

impl Queue<Mopac> for Slurm {
    fn write_submit_script(
        &self,
        infiles: impl IntoIterator<Item = String>,
        filename: &str,
    ) {
        let mut body = self
            .template
            .clone()
            .unwrap_or_else(|| {
                <Self as Queue<Mopac>>::default_submit_script(self)
            })
            .replace("{{.filename}}", filename);
        for f in infiles {
            body.push_str(&format!(
                "/home/qc/mopac2016/MOPAC2016.exe {f}.mop\n"
            ));
        }
        let mut file = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("write_submit_script: failed to create {filename}");
                std::process::exit(1);
            }
        };
        write!(file, "{body}").expect("failed to write params file");
    }

    fn default_submit_script(&self) -> String {
        "#!/bin/bash
#SBATCH --job-name=semp
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=1
#SBATCH -o {{.filename}}.out
#SBATCH --no-requeue
#SBATCH --mem=1gb
export LD_LIBRARY_PATH=/home/qc/mopac2016/
echo $SLURM_JOB_ID
date
hostname\n"
            .to_owned()
    }
}

impl Queue<DFTBPlus> for Slurm {
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

impl<P> SubQueue<P> for Slurm
where
    P: Program + Clone + Serialize + for<'a> Deserialize<'a>,
{
    fn submit_command(&self) -> &str {
        "sbatch"
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    fn job_limit(&self) -> usize {
        self.job_limit
    }

    fn sleep_int(&self) -> usize {
        self.sleep_int
    }

    const SCRIPT_EXT: &'static str = "slurm";

    fn dir(&self) -> &str {
        self.dir
    }

    /// run `squeue -u $USER`. form of the output is:
    ///
    ///    JOBID PARTITION   NAME     USER ST        TIME  NODES NODELIST(REASON)
    /// 30627992   compute  c3oh-   mdavis  R 46-17:12:23      1 node2
    fn stat_cmd(&self) -> String {
        let user = std::env::var("USER").expect("couldn't find $USER env var");
        let status = match std::process::Command::new("squeue")
            .args(["-u", &user])
            .output()
        {
            Ok(status) => status,
            Err(e) => panic!("failed to run squeue with {e}"),
        };
        String::from_utf8(status.stdout)
            .expect("failed to convert squeue output to String")
    }

    fn status(&self) -> HashSet<String> {
        let mut ret = HashSet::new();
        // wut?
        let lines = <Slurm as SubQueue<P>>::stat_cmd(self);
        let lines = lines.lines();
        for line in lines {
            if !line.contains("JOBID") {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert!(fields.len() == 8);
                // exclude completing jobs to combat stuck completing bug
                if fields[4] != "CG" {
                    ret.insert(fields[0].to_string());
                }
            }
        }
        ret
    }

    fn no_del(&self) -> bool {
        self.no_del
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    fn slurm() -> Slurm {
        Slurm {
            chunk_size: 1,
            job_limit: 1,
            sleep_int: 1,
            dir: "/tmp",
            no_del: false,
            template: None,
        }
    }

    macro_rules! make_tests {
        ($($name:ident, $queue:expr => $q:ty, $p:ty$(,)*)*) => {
            $(
            #[test]
            fn $name() {
                let tmp = tempfile::NamedTempFile::new().unwrap();
                <$q as Queue<$p>>::write_submit_script(
                    $queue,
                    ["opt0.inp", "opt1.inp", "opt2.inp", "opt3.inp"].map(|s| s.into()),
                    tmp.path().to_str().unwrap(),
                );
                let got = std::fs::read_to_string(tmp).unwrap();
                let got: Vec<&str> = got.lines().filter(|l|
                    !(l.starts_with("#SBATCH --job-name")
                        || l.starts_with("#SBATCH -o"))).collect();
                let got = got.join("\n");
                assert_snapshot!(got);
            }
            )*
        }
    }

    make_tests! {
        mopac_slurm, &slurm() => Slurm, Mopac,
        molpro_slurm, &slurm() => Slurm, Molpro,
        // cfour_slurm, &slurm() => Slurm, Cfour,
        // dftb_slurm, &slurm() => Slurm, DFTBPlus,
    }
}
