pub mod mopac;

// TODO these should maybe be Options or even an Enum, but it makes the API
// pretty painful
pub struct ProgramResult {
    pub energy: f64,
    pub cart_geom: Vec<Vec<f64>>,
}

#[derive(Debug, PartialEq)]
pub enum ProgramStatus {
    FileNotFound,
    ErrorInOutput,
    EnergyNotFound,
    EnergyParseError,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Procedure {
    Opt,
    Freq,
    SinglePt,
}

#[derive(Debug)]
pub struct Template<'a> {
    header: &'a str,
}

impl Template<'static> {
    pub const fn from(s: &'static str) -> Self {
        Self { header: s }
    }
}

pub trait Program {
    fn filename(&self) -> String;

    fn set_filename(&mut self, filename: &str);

    /// the template for writing input files
    fn template(&self) -> &Template;

    fn extension(&self) -> String;

    fn charge(&self) -> isize;

    fn write_input(&mut self, proc: Procedure);

    fn read_output(&self) -> Result<ProgramResult, ProgramStatus>;

    /// Return all the filenames associated with the Program for deletion when
    /// it finishes
    fn associated_files(&self) -> Vec<String>;
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
