use core::time;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
    str, thread,
};

use crate::{
    atom::Geom,
    program::{Procedure, Program, ProgramStatus},
};
use crate::{dump::Dump, program::Job};

pub mod local;
pub mod slurm;

static DEBUG: bool = false;

#[derive(PartialEq, Debug)]
pub struct Resubmit {
    pub inp_file: String,
    pub pbs_file: String,
    pub job_id: String,
}

pub trait Queue<P>
where
    P: Program + Clone,
{
    /// the extension to append to submit scripts for this type of Queue
    const SCRIPT_EXT: &'static str;

    fn dir(&self) -> &str;

    fn write_submit_script(&self, infiles: &[String], filename: &str);

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

    /// return a HashSet of jobs found in the queue based on the output of
    /// `stat_cmd`
    fn status(&self) -> HashSet<String> {
        let mut ret = HashSet::new();
        let lines = self.stat_cmd();
        let lines = lines.lines();
        for line in lines {
            if !line.contains("JOBID") {
                ret.insert(line.split_whitespace().next().unwrap().to_string());
            }
        }
        ret
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
        e: ProgramStatus,
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

    /// optimize is a stripped-down version of `drain` that runs a single job
    /// and returns the optimized geometry
    fn optimize(&self, job: Job<P>) -> Geom {
        let mut slurm_jobs = HashMap::new();
        let mut qstat = HashSet::<String>::new();
        let mut jobs = [job];
        self.build_chunk(&mut jobs, 0, &mut slurm_jobs, Procedure::Opt);
        qstat.insert(jobs[0].job_id.clone());
        loop {
            match jobs[0].program.read_output() {
                Ok(res) => return Geom::Xyz(res.cart_geom),
                Err(e) => self.drain_err_case(
                    e,
                    &mut qstat,
                    &mut slurm_jobs,
                    &mut jobs[0],
                ),
            }
            // only reached if unsuccessful so sleep
            qstat = self.status();
            thread::sleep(time::Duration::from_secs(self.sleep_int() as u64));
        }
    }

    fn drain(&self, jobs: &mut [Job<P>], dst: &mut [f64]) {
        let mut chunk_num: usize = 0;
        let mut cur_jobs = Vec::new();
        let mut slurm_jobs = HashMap::new();
        let mut remaining = jobs.len();
        let mut dump = Dump::new(self.chunk_size() * 5);
        let mut qstat = HashSet::<String>::new();
        let mut chunks = jobs.chunks_mut(self.chunk_size());
        let mut out_of_jobs = false;
        loop {
            // build more jobs if there is room
            while cur_jobs.len() < self.job_limit() {
                match chunks.next() {
                    Some(jobs) => {
                        self.build_chunk(
                            jobs,
                            chunk_num,
                            &mut slurm_jobs,
                            Procedure::SinglePt,
                        );
                        let job_id = jobs[0].job_id.clone();
                        qstat.insert(job_id);
                        if DEBUG {
                            eprintln!("submitted chunk {}", chunk_num);
                        }
                        chunk_num += 1;
                        cur_jobs.extend(jobs);
                    }
                    None => {
                        out_of_jobs = true;
                        break;
                    }
                }
            }
            // collect output
            let mut finished = 0;
            let mut to_remove = Vec::new();
            for (i, job) in cur_jobs.iter_mut().enumerate() {
                match job.program.read_output() {
                    Ok(res) => {
                        to_remove.push(i);
                        dst[job.index] += job.coeff * res.energy;
                        dump.add(job.program.associated_files());
                        finished += 1;
                        remaining -= 1;
                        let job_name = job.pbs_file.as_str();
                        let mut count = match slurm_jobs.get_mut(job_name) {
                            Some(n) => *n,
                            None => {
                                eprintln!(
                                    "failed to find {} in slurm_jobs",
                                    job_name
                                );
                                1
                            }
                        };
                        count -= 1;
                        if count == 0 {
                            // delete the submit script
                            dump.add(vec![
                                job_name.to_string(),
                                format!("{}.out", job_name),
                            ]);
                        }
                    }
                    Err(e) => {
                        self.drain_err_case(
                            e,
                            &mut qstat,
                            &mut slurm_jobs,
                            job,
                        );
                    }
                }
            }
            // have to remove the highest index first so sort and reverse
            to_remove.sort();
            to_remove.reverse();
            for i in to_remove {
                cur_jobs.swap_remove(i);
            }
            if cur_jobs.len() == 0 && out_of_jobs {
                return;
            }
            if finished == 0 {
                eprintln!("{} jobs remaining", remaining);
                qstat = self.status();
                thread::sleep(time::Duration::from_secs(
                    self.sleep_int() as u64
                ));
            } else if finished > remaining / 10 {
                eprintln!("{} jobs remaining", remaining);
            }
        }
    }
}
