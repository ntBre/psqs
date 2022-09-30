use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
    str,
};

use crate::program::{Job, ProgramResult};
use crate::{
    geom::Geom,
    program::{Procedure, Program, ProgramError},
};

pub mod local;
pub mod slurm;
use drain::*;
mod drain;

static DEBUG: bool = false;

#[derive(PartialEq, Eq, Debug)]
pub struct Resubmit {
    pub inp_file: String,
    pub pbs_file: String,
    pub job_id: String,
}

/// a trait for all of the program-independent parts of a [Queue]
pub trait SubQueue<P>
where
    P: Program + Clone,
{
    /// the extension to append to submit scripts for this type of Queue
    const SCRIPT_EXT: &'static str;

    fn dir(&self) -> &str;

    fn submit_command(&self) -> &str;

    fn chunk_size(&self) -> usize;

    fn job_limit(&self) -> usize;

    fn sleep_int(&self) -> usize;

    /// submit `filename` to the queue and return the jobid
    fn submit(&self, filename: &str) -> String {
        match Command::new(self.submit_command()).arg(filename).output() {
            Ok(s) => {
                let raw = str::from_utf8(&s.stdout).unwrap().trim().to_string();
                return raw.split_whitespace().last().unwrap().to_string();
            }
            Err(_) => todo!(),
        };
    }

    /// the command to check the status of jobs in the queue
    fn stat_cmd(&self) -> String;

    /// return a HashSet of jobs found in the queue based on the output of
    /// `stat_cmd`
    fn status(&self) -> HashSet<String>;
}

pub trait Queue<P>: SubQueue<P>
where
    P: Program + Clone,
{
    fn write_submit_script(&self, infiles: &[String], filename: &str);

    /// take a name of a Program input file with the extension attached, replace
    /// the extension (ext) with _redo.ext and write _redo.SCRIPT_EXT, then
    /// submit the redo script
    fn resubmit(&self, filename: &str) -> Resubmit {
        let path = Path::new(filename);
        let dir = path.parent().unwrap().to_str().unwrap();
        let base = path.file_stem().unwrap().to_str().unwrap();
        {
            let ext = path.extension().unwrap().to_str().unwrap();
            let inp_file = format!("{}/{}_redo.{}", dir, base, ext);
            match std::fs::copy(filename, &inp_file) {
                Ok(_) => (),
                Err(e) => {
                    panic!("failed to copy {filename} to {inp_file} with `{e}`")
                }
            };
        }
        // nothing but the copy needs the name with extension
        let inp_name = format!("{}/{}_redo", dir, base);
        let pbs_file = format!("{}/{}_redo.{}", dir, base, Self::SCRIPT_EXT);
        self.write_submit_script(&[inp_name.clone()], &pbs_file);
        let job_id = self.submit(&pbs_file);
        Resubmit {
            inp_file: inp_name,
            pbs_file,
            job_id,
        }
    }

    /// Build a chunk of jobs by writing the Program input file and the
    /// corresponding submission script and then submitting the script
    fn build_chunk<'a>(
        &self,
        jobs: &mut [Job<P>],
        chunk_num: usize,
        slurm_jobs: &'a mut HashMap<String, usize>,
        proc: Procedure,
    ) {
        let queue_file =
            format!("{}/main{}.{}", self.dir(), chunk_num, Self::SCRIPT_EXT);
        let jl = jobs.len();
        let mut filenames = Vec::with_capacity(jl);
        for job in &mut *jobs {
            job.program.write_input(proc);
            job.pbs_file = queue_file.to_string();
            filenames.push(job.program.filename());
        }
        slurm_jobs.insert(queue_file.clone(), jl);
        self.write_submit_script(&filenames, &queue_file);
        // run jobs
        let job_id = self.submit(&queue_file);
        for mut job in jobs {
            job.job_id = job_id.clone();
        }
    }

    fn drain_err_case(
        &self,
        e: ProgramError,
        qstat: &mut HashSet<String>,
        slurm_jobs: &mut HashMap<String, usize>,
        job: &mut Job<P>,
    ) {
        // just overwrite the existing job with the resubmitted
        // version
        if !qstat.contains(&job.job_id) {
            eprintln!("resubmitting {} for {:?}", job.program.filename(), e);
            let resub = format!(
                "{}.{}",
                job.program.filename(),
                job.program.extension()
            );
            let Resubmit {
                inp_file,
                pbs_file,
                job_id,
            } = self.resubmit(&resub);
            job.program.set_filename(&inp_file);
            job.pbs_file = pbs_file.clone();
            slurm_jobs.insert(pbs_file, 1);
            qstat.insert(job_id.clone());
            job.job_id = job_id;
        }
    }

    /// optimize is a copy of drain for optimizing jobs
    fn optimize(
        &self,
        jobs: &mut [Job<P>],
        dst: &mut [Geom],
    ) -> Result<(), ProgramError> {
        Opt.drain(self, jobs, dst)
    }

    fn drain(
        &self,
        jobs: &mut [Job<P>],
        dst: &mut [f64],
    ) -> Result<(), ProgramError> {
        Single.drain(self, jobs, dst)
    }

    fn energize(
        &self,
        jobs: &mut [Job<P>],
        dst: &mut [ProgramResult],
    ) -> Result<(), ProgramError> {
        Both.drain(self, jobs, dst)
    }
}
