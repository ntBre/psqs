use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

use crate::program::molpro::Molpro;
use crate::program::mopac::Mopac;
use crate::program::Program;
use crate::queue::Queue;

use super::SubQueue;

/// Pbs is a type for holding the information for submitting a pbs job.
/// `filename` is the name of the Pbs submission script
#[derive(Debug)]
pub struct Pbs {
    chunk_size: usize,
    job_limit: usize,
    sleep_int: usize,
    dir: &'static str,
}

impl Pbs {
    pub fn new(
        chunk_size: usize,
        job_limit: usize,
        sleep_int: usize,
        dir: &'static str,
    ) -> Self {
        Self {
            chunk_size,
            job_limit,
            sleep_int,
            dir,
        }
    }

    pub fn default() -> Self {
        Self {
            chunk_size: 128,
            job_limit: 1600,
            sleep_int: 5,
            dir: "inp",
        }
    }
}

impl Queue<Molpro> for Pbs {
    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        // TODO I'm going to have to split the filename again for maple
        let mut body = format!(
            "#!/bin/sh
#PBS -N {filename}
#PBS -S /bin/bash
#PBS -j oe
#PBS -o {filename}.out
#PBS -W umask=022
#PBS -l walltime=9999:00:00
#PBS -l ncpus=1
#PBS -l mem=8gb
#PBS -q workq

module load openpbs molpro

export WORKDIR=$PBS_O_WORKDIR
export TMPDIR=/tmp/$USER/$PBS_JOBID
cd $WORKDIR
mkdir -p $TMPDIR

"
        );
        {
            use std::fmt::Write;
            for f in infiles {
                writeln!(body, "molpro -t 1 --no-xml-output {f}.inp").unwrap();
            }
            writeln!(body, "rm -rf $TMPDIR").unwrap();
        }
        let mut file = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                panic!("write_submit_script: failed to create {filename}");
            }
        };
        write!(file, "{}", body).unwrap_or_else(|_| {
            panic!("failed to write molpro input file: {}", filename)
        });
    }
}

impl Queue<Mopac> for Pbs {
    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        let mut body = format!(
            "#!/bin/sh
#PBS -N {filename}
#PBS -S /bin/bash
#PBS -j oe
#PBS -o {filename}.out
#PBS -W umask=022
#PBS -l walltime=9999:00:00
#PBS -l ncpus=1
#PBS -l mem=1gb
#PBS -q workq

module load openpbs

export WORKDIR=$PBS_O_WORKDIR
cd $WORKDIR

",
        );
        for f in infiles {
            body.push_str(&format!(
                "/ddn/home1/r2518/.conda/envs/mopac/bin/mopac {f}.mop\n"
            ));
        }
        let mut file = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("write_submit_script: failed to create {filename}");
                std::process::exit(1);
            }
        };
        write!(file, "{}", body).expect("failed to write params file");
    }
}

impl<P: Program + Clone> SubQueue<P> for Pbs {
    fn submit_command(&self) -> &str {
        "qsub"
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

    const SCRIPT_EXT: &'static str = "pbs";

    fn dir(&self) -> &str {
        self.dir
    }

    /// run `qstat -u $USER`. form of the output is:
    ///
    /// maple:
    ///                                                             Req'd  Req'd   Elap
    /// Job ID          Username Queue    Jobname    SessID NDS TSK Memory Time  S Time
    /// --------------- -------- -------- ---------- ------ --- --- ------ ----- - -----
    /// 819446          user     queue    C6HNpts      5085   1   1    8gb 26784 R 00:00
    fn stat_cmd(&self) -> String {
        let user = std::env::vars()
            .find(|x| x.0 == "USER")
            .expect("couldn't find $USER env var");
        let status = match std::process::Command::new("qstat")
            .args(["-u", &user.1])
            .output()
        {
            Ok(status) => status,
            Err(e) => panic!("failed to run squeue with {}", e),
        };
        String::from_utf8(status.stdout)
            .expect("failed to convert squeue output to String")
    }

    fn status(&self) -> HashSet<String> {
        let mut ret = HashSet::new();
        let lines = <Pbs as SubQueue<P>>::stat_cmd(self);
        // skip to end of header
        let lines = lines.lines().skip_while(|l| !l.contains("-----------"));
        for line in lines {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert!(fields.len() == 11);
            ret.insert(fields[0].to_string());
        }
        ret
    }
}