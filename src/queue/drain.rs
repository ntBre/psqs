use core::time;
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    thread,
    time::Duration,
};

use crate::{
    geom::Geom,
    program::{Job, Procedure, Program, ProgramError, ProgramResult},
    queue::drain::dump::Dump,
};

use super::{Queue, DEBUG};

macro_rules! time {
    ($msg:expr, $body:block) => {
        let now = std::time::Instant::now();
        $body;
        eprintln!(
            "finished {} after {:.1} s",
            $msg,
            now.elapsed().as_millis() as f64 / 1000.0
        );
    };
}

mod dump;

#[derive(Default)]
struct Time {
    writing: Duration,
    reading: Duration,
    sleeping: Duration,
    removing: Duration,
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.1} s reading ok, {:.1} s writing, {:.1} s sleeping, \
	     {:.1} s removing",
            self.reading.as_millis() as f64 / 1000.0,
            self.writing.as_millis() as f64 / 1000.0,
            self.sleeping.as_millis() as f64 / 1000.0,
            self.removing.as_millis() as f64 / 1000.0,
        )
    }
}

/// a histogram covering the range [min, max) with `N` bins
struct Histogram<const N: usize> {
    min: f64,
    cur_min: f64,
    cur_max: f64,
    total: f64,
    denom: f64,
    data: [usize; N],
}

impl<const N: usize> Histogram<N> {
    fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            denom: max - min,
            data: [0; N],
            cur_min: 0.0,
            cur_max: 0.0,
            total: 0.0,
        }
    }

    /// insert `val` into the appropriate bin in `self` and add it to the total.
    /// if `val` is greater than `self.max`, don't perform the insert but add it
    /// to the other statistics
    fn insert(&mut self, val: f64) {
        let idx = N as f64 * (val - self.min) / self.denom;
        if let Some(elt) = self.data.get_mut(idx.floor() as usize) {
            *elt += 1;
        }
        if val > self.cur_max {
            self.cur_max = val;
        }
        if val < self.cur_min {
            self.cur_min = val;
        }
        self.total += val;
    }

    /// return the count of elements in `self`
    fn count(&self) -> usize {
        self.data.iter().sum()
    }

    /// return the average of `self`
    fn average(&self) -> f64 {
        self.total / self.count() as f64
    }
}

impl<const N: usize> Display for Histogram<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bin_width = self.denom / N as f64;
        for (i, v) in self.data.iter().enumerate() {
            if *v > 0 {
                writeln!(f, "{:5.2}{:5}", i as f64 * bin_width, v)?;
            }
        }
        Ok(())
    }
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

    fn drain<P: Program + Clone, Q: Queue<P> + ?Sized>(
        &self,
        dir: &str,
        queue: &Q,
        jobs: &mut [Job<P>],
        dst: &mut [Self::Item],
    ) -> Result<(), ProgramError> {
        let mut chunk_num: usize = 0;
        let mut cur_jobs = Vec::new();
        let mut slurm_jobs = HashMap::new();
        let mut remaining = jobs.len();

        let dump = Dump::new();
        let mut time = Time::default();

        // histogram for tracking job times
        let mut job_time = Histogram::<100>::new(0.0, 10.0);

        let mut qstat = HashSet::<String>::new();
        let mut chunks = jobs.chunks_mut(queue.chunk_size());
        let mut out_of_jobs = false;
        let mut to_remove = Vec::new();
        loop {
            let loop_time = std::time::Instant::now();
            // build more jobs if there is room
            while !out_of_jobs && cur_jobs.len() < queue.job_limit() {
                match chunks.next() {
                    Some(jobs) => {
                        let now = std::time::Instant::now();
                        queue.build_chunk(
                            dir,
                            jobs,
                            chunk_num,
                            &mut slurm_jobs,
                            self.procedure(),
                        );
                        let job_id = jobs[0].job_id.clone();
                        qstat.insert(job_id);
                        let elapsed = now.elapsed();
                        if DEBUG {
                            eprintln!(
                                "submitted chunk {} after {:.1} s",
                                chunk_num,
                                elapsed.as_millis() as f64 / 1000.0
                            );
                        }
                        time.writing += elapsed;
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
                        job_time.insert(res.time);
                        self.set_result(dst, *job, res);
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
                        queue.drain_err_case(
                            e,
                            &mut qstat,
                            &mut slurm_jobs,
                            job,
                        );
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
                eprintln!("total job time: {:.2} s", job_time.total);
                eprintln!("max job time: {:.2} s", job_time.cur_max);
                eprintln!("min job time: {:.2} s", job_time.cur_min);
                eprintln!("avg job time: {:.2} s", job_time.average());
                eprint!("histogram:\n{}", job_time);
                return Ok(());
            }
            if finished == 0 {
                eprintln!("{} jobs remaining", remaining);
                qstat = queue.status();
                let d = time::Duration::from_secs(queue.sleep_int() as u64);
                time.sleeping += d;
                thread::sleep(d);
            }
        }
    }
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
