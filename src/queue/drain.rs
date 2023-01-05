use core::time;
use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    marker::{Send, Sync},
    thread,
};

use crate::{
    geom::Geom,
    program::{Job, Procedure, Program, ProgramError, ProgramResult},
    queue::drain::{dump::Dump, resub::ResubOutput},
};

use super::{Queue, DEBUG};

/// time the duration of `$body` and store the resulting Duration in `$elapsed`
#[macro_export]
macro_rules! time {
    ($elapsed:ident, $body:block) => {
        let now = std::time::Instant::now();
        $body;
        let $elapsed = now.elapsed();
    };
}

mod dump;
mod resub;
mod timer;

use lazy_static::lazy_static;
use resub::Resub;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref NO_RESUB: bool = std::env::var("NO_RESUB").is_ok();
}

pub(crate) trait Drain {
    type Item;

    fn procedure(&self) -> Procedure;

    fn set_result<P: Program>(
        &self,
        dst: &mut [Self::Item],
        job: &mut Job<P>,
        res: ProgramResult,
    );

    /// on success, return the total job time, as returned by `P::read_output`
    fn drain<P, Q>(
        &self,
        dir: &str,
        queue: &Q,
        mut jobs: Vec<Job<P>>,
        dst: &mut [Self::Item],
        check_int: usize,
    ) -> Result<f64, ProgramError>
    where
        Self: Sync,
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
        Q: Queue<P> + ?Sized + Sync,
        <Self as Drain>::Item: Clone,
    {
        // total time for the jobs to run as returned from Program::read_output
        let mut job_time = 0.0;

        let mut cur_jobs = Vec::new();
        let mut slurm_jobs = HashMap::new();
        let mut remaining = jobs.len();

        let job_limit = queue.job_limit();

        let mut out_of_jobs = false;

        let dump = Dump::new();
        let mut time = timer::Timer::default();

        let mut qstat = HashSet::<String>::new();
        let mut chunks = jobs
            .chunks_mut(queue.chunk_size())
            .enumerate()
            .fuse()
            .peekable();
        let mut to_remove = Vec::new();
        let mut resub = Resub::new(queue, dir, self.procedure());
        let mut iter = 1;
        loop {
            let loop_time = std::time::Instant::now();
            if chunks.peek().is_none() {
                out_of_jobs = true;
            }
            if !out_of_jobs {
                let works: Vec<_> = chunks
                    .borrow_mut()
                    .take(job_limit - cur_jobs.len() / queue.chunk_size())
                    .par_bridge()
                    .map(|(chunk_num, jobs)| {
                        let now = std::time::Instant::now();
                        let (slurm_jobs, wi, ws, ss) = queue.build_chunk(
                            dir,
                            jobs,
                            chunk_num,
                            self.procedure(),
                        );
                        let job_id = jobs[0].job_id.clone();
                        let elapsed = now.elapsed();
                        if DEBUG {
                            eprintln!(
                                "submitted chunk {} after {:.1} s",
                                chunk_num,
                                elapsed.as_millis() as f64 / 1000.0
                            );
                        }
                        (jobs.to_vec(), slurm_jobs, job_id, wi, ws, ss)
                    })
                    .collect();
                for (jobs, sj, job_id, wi, ws, ss) in works {
                    slurm_jobs.extend(sj);
                    time.writing_input += wi;
                    time.writing_script += ws;
                    time.submitting_script += ss;
                    qstat.insert(job_id);
                    cur_jobs.extend(jobs);
                }
            }

            // collect output
            let mut finished = 0;
            to_remove.clear();
            let now = std::time::Instant::now();
            let outfiles: Vec<_> =
                cur_jobs.iter().map(|job| job.program.filename()).collect();
            use rayon::prelude::*;
            let results: Vec<_> =
                outfiles.par_iter().map(|out| P::read_output(out)).collect();
            time.reading += now.elapsed();
            for (i, (job, res)) in cur_jobs.iter_mut().zip(results).enumerate()
            {
                match res {
                    Ok(res) => {
                        to_remove.push(i);
                        job_time += res.time;
                        self.set_result(dst, job, res);
                        for f in job.program.associated_files() {
                            dump.send(f);
                        }
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
                            // delete the submit script and output file
                            dump.send(job_name.to_string());
                            dump.send(format!("{}.out", job_name));
                        }
                    }
                    Err(e) => {
                        if e.is_error_in_output() {
                            dump.shutdown();
                            return Err(e);
                        }
                        // just overwrite the existing job with the resubmitted
                        // version
                        if !qstat.contains(&job.job_id) {
                            let time = job.modtime();
                            if time > job.modtime {
                                // file has been updated since we last looked at it, so need to
                                // look again
                                job.modtime = time;
                            } else {
                                // actual resubmission path
                                eprintln!(
                                    "resubmitting {} (id={}) for {:?}",
                                    job.program.filename(),
                                    job.job_id,
                                    e
                                );
                                if *NO_RESUB {
                                    eprintln!(
                                        "resubmission disabled by \
					 NO_RESUB environment \
					 variable, exiting"
                                    );
                                    std::process::exit(1);
                                }
                                // copy the job into resub and plan to remove it
                                // from cur_jobs
                                resub.push(job.clone());
                                to_remove.push(i);
                            }
                        };
                    }
                }
            }
            // have to remove the highest index first so sort and reverse
            let r = std::time::Instant::now();
            to_remove.sort();
            to_remove.reverse();
            for i in &to_remove {
                cur_jobs.swap_remove(*i);
            }
            time.removing += r.elapsed();
            // submit resubs
            let works = resub.resubmit();
            for ResubOutput {
                jobs,
                slurm_jobs: sj,
                job_id,
                writing_input: wi,
                writing_script: ws,
                submitting: ss,
            } in works
            {
                slurm_jobs.extend(sj);
                time.writing_input += wi;
                time.writing_script += ws;
                time.submitting_script += ss;
                qstat.insert(job_id);
                cur_jobs.extend(jobs);
            }
            if DEBUG {
                eprintln!(
                    "finished {} jobs in {:.1} s",
                    finished,
                    loop_time.elapsed().as_millis() as f64 / 1000.0
                );
            }
            if cur_jobs.is_empty() && out_of_jobs {
                dump.shutdown();
                eprintln!("{}", time);
                return Ok(job_time);
            }
            if finished == 0 {
                eprintln!("{} jobs remaining", remaining);
                qstat = queue.status();
                let d = time::Duration::from_secs(queue.sleep_int() as u64);
                time.sleeping += d;
                thread::sleep(d);
            }
            if check_int > 0 && check_int % iter == 0 {
                Self::write_checkpoint(
                    "chk.json",
                    dst.to_vec(),
                    cur_jobs.clone(),
                );
            }
            iter += 1;
        }
    }

    fn load_checkpoint<P>(
        checkpoint: &str,
        dst: &mut [Self::Item],
    ) -> Vec<Job<P>>
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>;

    fn write_checkpoint<P>(
        checkpoint: &str,
        dst: Vec<Self::Item>,
        jobs: Vec<Job<P>>,
    ) where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>;
}

pub(crate) struct Opt;

impl Drain for Opt {
    type Item = Geom;

    fn procedure(&self) -> Procedure {
        Procedure::Opt
    }

    fn set_result<P: Program>(
        &self,
        dst: &mut [Self::Item],
        job: &mut Job<P>,
        res: ProgramResult,
    ) {
        dst[job.index] = Geom::Xyz(res.cart_geom.unwrap());
    }

    fn load_checkpoint<P>(
        _checkpoint: &str,
        _dst: &mut [Self::Item],
    ) -> Vec<Job<P>>
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        todo!()
    }

    fn write_checkpoint<P>(
        _checkpoint: &str,
        _dst: Vec<Self::Item>,
        _jobs: Vec<Job<P>>,
    ) where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        todo!()
    }
}

#[derive(Deserialize, Serialize)]
struct Checkpoint<P>
where
    P: Program + Clone,
{
    dst: Vec<f64>,
    jobs: Vec<Job<P>>,
}

pub(crate) struct Single;

impl Drain for Single {
    type Item = f64;

    fn procedure(&self) -> Procedure {
        Procedure::SinglePt
    }

    fn set_result<P: Program>(
        &self,
        dst: &mut [Self::Item],
        job: &mut Job<P>,
        res: ProgramResult,
    ) {
        dst[job.index] += job.coeff * res.energy;
    }

    /// load a checkpoint from the `checkpoint` file, storing the energies in
    /// `dst` and returning the list of remaining jobs
    fn load_checkpoint<P>(checkpoint: &str, dst: &mut [f64]) -> Vec<Job<P>>
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        let f = std::fs::File::open(checkpoint).unwrap();
        let Checkpoint { dst: d, jobs } = serde_json::from_reader(f).unwrap();
        dst.copy_from_slice(&d);
        jobs
    }

    fn write_checkpoint<P>(checkpoint: &str, dst: Vec<f64>, jobs: Vec<Job<P>>)
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        let c = Checkpoint { dst, jobs };
        let f = std::fs::File::create(checkpoint).unwrap();
        serde_json::to_writer_pretty(f, &c).unwrap();
    }
}

pub(crate) struct Both;

impl Drain for Both {
    type Item = ProgramResult;

    fn procedure(&self) -> Procedure {
        Procedure::Opt
    }

    fn set_result<P: Program>(
        &self,
        dst: &mut [Self::Item],
        job: &mut Job<P>,
        res: ProgramResult,
    ) {
        dst[job.index] = res;
    }

    fn load_checkpoint<P>(
        _checkpoint: &str,
        _dst: &mut [Self::Item],
    ) -> Vec<Job<P>>
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        todo!()
    }

    fn write_checkpoint<P>(
        _checkpoint: &str,
        _dst: Vec<Self::Item>,
        _jobs: Vec<Job<P>>,
    ) where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    {
        todo!()
    }
}
