use core::time;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
    thread,
};

use crate::{
    geom::Geom,
    program::{Job, Procedure, Program, ProgramError, ProgramResult},
};

use super::{Queue, DEBUG};

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

        let (sender, receiver) = mpsc::channel();
        let dump = thread::spawn(move || {
            for file in receiver {
                let e = std::fs::remove_file(&file);
                if let Err(e) = e {
                    eprintln!("failed to remove {file} with {e}");
                }
            }
        });

        let mut qstat = HashSet::<String>::new();
        let mut chunks = jobs.chunks_mut(queue.chunk_size());
        let mut out_of_jobs = false;
        loop {
            // build more jobs if there is room
            while cur_jobs.len() < queue.job_limit() {
                match chunks.next() {
                    Some(jobs) => {
                        queue.build_chunk(
                            dir,
                            jobs,
                            chunk_num,
                            &mut slurm_jobs,
                            self.procedure(),
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
                        self.set_result(dst, *job, res);
                        // dump.add(job.program.associated_files());
                        for f in job.program.associated_files() {
                            sender.send(f).unwrap();
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
                            sender.send(job_name.to_string()).unwrap();
                            sender.send(format!("{}.out", job_name)).unwrap();
                        }
                    }
                    Err(e) => {
                        if e.is_error_in_output() {
                            drop(sender);
                            dump.join().unwrap();
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
            to_remove.sort();
            to_remove.reverse();
            for i in to_remove {
                cur_jobs.swap_remove(i);
            }
            if cur_jobs.is_empty() && out_of_jobs {
                drop(sender);
                dump.join().unwrap();
                return Ok(());
            }
            if finished == 0 {
                eprintln!("{} jobs remaining", remaining);
                qstat = queue.status();
                thread::sleep(time::Duration::from_secs(
                    queue.sleep_int() as u64
                ));
            } else if finished > remaining / 10 {
                eprintln!("{} jobs remaining", remaining);
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
