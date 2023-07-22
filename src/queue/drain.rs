use core::time;
use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    iter::Peekable,
    marker::{Send, Sync},
    sync::LazyLock,
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

use libc::{timeval, RUSAGE_SELF};
use resub::Resub;
use serde::{Deserialize, Serialize};

static NO_RESUB: LazyLock<bool> =
    LazyLock::new(|| std::env::var("NO_RESUB").is_ok());

pub enum Check {
    Some { check_int: usize, check_dir: String },
    None,
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
        jobs: impl Iterator<Item = Job<P>>,
        dst: &mut [Self::Item],
        check: Check,
    ) -> Result<f64, ProgramError>
    where
        Self: Sync,
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
        Q: Queue<P> + ?Sized + Sync,
        <Self as Drain>::Item: Clone + Serialize,
    {
        // total time for the jobs to run as returned from Program::read_output
        let mut job_time = 0.0;

        let mut cur_jobs = Vec::new();
        let mut slurm_jobs = HashMap::new();

        let job_limit = queue.job_limit();

        let mut out_of_jobs = false;

        let dump = Dump::new(queue.no_del());
        let mut time = timer::Timer::default();

        let mut qstat = HashSet::<String>::new();
        // the index of the last chunk consumed. used for writing remaining jobs
        // to checkpoints. None initially and then Some(chunk_num)
        let mut to_remove = Vec::new();
        let mut resub = Resub::new(queue, dir, self.procedure());
        let mut iter = 0;

        let mut jobs = jobs.fuse().enumerate().peekable();
        loop {
            let loop_time = std::time::Instant::now();
            if jobs.peek().is_none() {
                out_of_jobs = true;
            }
            if !out_of_jobs {
                self.receive_jobs(
                    &mut jobs,
                    job_limit,
                    &mut cur_jobs,
                    queue,
                    dir,
                    &mut slurm_jobs,
                    &mut time,
                    &mut qstat,
                );
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
                        let job_name = job.pbs_file.as_str();
                        let mut count = match slurm_jobs.get_mut(job_name) {
                            Some(n) => *n,
                            None => {
                                eprintln!(
                                    "failed to find {job_name} in slurm_jobs"
                                );
                                1
                            }
                        };
                        count -= 1;
                        if count == 0 {
                            // delete the submit script and output file
                            dump.send(job_name.to_string());
                            dump.send(format!("{job_name}.out"));
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
                                // file has been updated since we last looked at
                                // it, so need to look again
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
                eprintln!("{time}");
                return Ok(job_time);
            }
            if finished == 0 {
                wait(queue, &mut time, iter);
                qstat = queue.status();
            }

            if let Check::Some {
                check_int,
                check_dir,
            } = &check
            {
                if *check_int > 0 && iter % check_int == 0 {
                    let cur_jobs = cur_jobs.clone();
                    Self::write_checkpoint(
                        &format!("{check_dir}/chk.json"),
                        dst.to_vec(),
                        cur_jobs,
                    );
                }
            }
            iter += 1;
        }
    }

    /// load a checkpoint from the `checkpoint` file, storing the energies in
    /// `dst` and returning the list of remaining jobs
    fn load_checkpoint<P>(
        checkpoint: &str,
        dst: &mut [Self::Item],
    ) -> Vec<Job<P>>
    where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
        Self::Item: Clone + for<'a> Deserialize<'a>,
    {
        let Ok(f) = std::fs::File::open(checkpoint) else {
            panic!("failed to open {checkpoint}");
        };
        let Checkpoint { dst: d, jobs } = serde_json::from_reader(f).unwrap();
        dst.clone_from_slice(&d);
        jobs
    }

    fn write_checkpoint<P>(
        checkpoint: &str,
        dst: Vec<Self::Item>,
        jobs: Vec<Job<P>>,
    ) where
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
        Self::Item: Serialize,
    {
        let c = Checkpoint { dst, jobs };
        eprintln!("writing checkpoint to {checkpoint}");
        let f = std::fs::File::create(checkpoint).unwrap();
        serde_json::to_writer_pretty(f, &c).unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn receive_jobs<P, Q>(
        &self,
        jobs: &mut Peekable<impl Iterator<Item = (usize, Job<P>)>>,
        job_limit: usize,
        cur_jobs: &mut Vec<Job<P>>,
        queue: &Q,
        dir: &str,
        slurm_jobs: &mut HashMap<String, usize>,
        time: &mut timer::Timer,
        qstat: &mut HashSet<String>,
    ) where
        Self: Sync,
        P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
        Q: Queue<P> + ?Sized + Sync,
        <Self as Drain>::Item: Clone + Serialize,
    {
        let mut chunks = Vec::new();
        let chunk_size = queue.chunk_size();
        while jobs.peek().is_some() && cur_jobs.len() + chunk_size <= job_limit
        {
            let chunk: Vec<_> =
                jobs.borrow_mut().take(chunk_size).map(|(_, j)| j).collect();
            chunks.push(chunk);
        }

        use rayon::prelude::*;
        let works: Vec<_> = chunks
            .into_iter()
            .enumerate()
            .par_bridge()
            .map(|(chunk_num, mut jobs)| {
                let now = std::time::Instant::now();
                let (slurm_jobs, wi, ws, ss) = queue.build_chunk(
                    dir,
                    &mut jobs,
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
}

fn to_secs(time: timeval) -> f64 {
    time.tv_sec as f64 + time.tv_usec as f64 / 1e6
}

/// return the CPU time used by the current process in seconds
fn get_cpu_time() -> f64 {
    unsafe {
        let mut rusage = std::mem::MaybeUninit::uninit();
        let res = libc::getrusage(RUSAGE_SELF, rusage.as_mut_ptr());
        if res != 0 {
            return 0.0;
        }
        let rusage = rusage.assume_init();
        to_secs(rusage.ru_stime) + to_secs(rusage.ru_utime)
    }
}

fn wait<P, Q>(queue: &Q, time: &mut timer::Timer, iter: usize)
where
    P: Program + Clone + Send + Sync + Serialize + for<'a> Deserialize<'a>,
    Q: Queue<P> + ?Sized + Sync,
{
    let date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    eprintln!("[iter {iter} {date} {:.1} CPU s]", get_cpu_time());
    let d = time::Duration::from_secs(queue.sleep_int() as u64);
    time.sleeping += d;
    thread::sleep(d);
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
}

#[derive(Deserialize, Serialize)]
struct Checkpoint<P, T>
where
    P: Program + Clone,
{
    dst: Vec<T>,
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
}
