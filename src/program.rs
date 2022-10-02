use symm::Atom;

use crate::geom::Geom;

pub mod molpro;
pub mod mopac;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramResult {
    pub energy: f64,
    pub cart_geom: Option<Vec<Atom>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramError {
    FileNotFound(String),
    ErrorInOutput(String),
    EnergyNotFound(String),
    EnergyParseError(String),
}

impl ProgramError {
    /// Returns `true` if the program error is [`ErrorInOutput`].
    ///
    /// [`ErrorInOutput`]: ProgramError::ErrorInOutput
    #[must_use]
    pub fn is_error_in_output(&self) -> bool {
        matches!(self, Self::ErrorInOutput(..))
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Procedure {
    Opt,
    Freq,
    SinglePt,
}

#[derive(Clone, Debug)]
pub struct Template {
    header: String,
}

impl Template {
    pub fn from(s: &str) -> Self {
        Self {
            header: s.to_string(),
        }
    }
}

/// A trait for describing programs runnable on a [crate::queue::Queue]
pub trait Program {
    /// returns the file associated with the program's input. it should not
    /// include an extension
    fn filename(&self) -> String;

    /// set `filename`
    fn set_filename(&mut self, filename: &str);

    /// the template for writing input files
    fn template(&self) -> &Template;

    /// the file extension for the input file
    fn extension(&self) -> String;

    /// molecular charge
    fn charge(&self) -> isize;

    /// write the input file to the name returned by `filename`
    fn write_input(&mut self, proc: Procedure);

    /// read the output file found by replacing `self.extension()` with `.out`
    fn read_output(&self) -> Result<ProgramResult, ProgramError>;

    /// Return all the filenames associated with the Program for deletion when
    /// it finishes
    fn associated_files(&self) -> Vec<String>;

    fn new(
        filename: String,
        template: Template,
        charge: isize,
        geom: Geom,
    ) -> Self;

    /// Build the jobs described by `moles` in memory, but don't write any of
    /// their files yet
    fn build_jobs(
        moles: &Vec<Geom>,
        dir: &'static str,
        start_index: usize,
        coeff: f64,
        job_num: usize,
        charge: isize,
        tmpl: Template,
    ) -> Vec<Job<Self>>
    where
        Self: std::marker::Sized,
    {
        let mut count: usize = start_index;
        let mut job_num = job_num;
        let mut jobs = Vec::new();
        for mol in moles {
            let filename = format!("{dir}/job.{:08}", job_num);
            job_num += 1;
            let mut job = Job::new(
                Self::new(filename, tmpl.clone(), charge, mol.clone()),
                count,
            );
            job.coeff = coeff;
            jobs.push(job);
            count += 1;
        }
        jobs
    }
}

#[derive(Debug, Clone)]
pub struct Job<P: Program> {
    pub program: P,
    pub pbs_file: String,
    pub job_id: String,

    /// the index in the output array to store the result
    pub index: usize,

    /// the coefficient to multiply by when storing the result
    pub coeff: f64,
}

impl<P: Program> Job<P> {
    pub fn new(program: P, index: usize) -> Self {
        Self {
            program,
            pbs_file: String::new(),
            job_id: String::new(),
            index,
            coeff: 1.0,
        }
    }
}
