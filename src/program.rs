use symm::Atom;

pub mod mopac;

// TODO these should maybe be Options or even an Enum, but it makes the API
// pretty painful
#[derive(Clone, Debug, Default)]
pub struct ProgramResult {
    pub energy: f64,
    pub cart_geom: Vec<Atom>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramError {
    FileNotFound,
    ErrorInOutput,
    EnergyNotFound,
    EnergyParseError,
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

pub trait Program {
    fn filename(&self) -> String;

    fn set_filename(&mut self, filename: &str);

    /// the template for writing input files
    fn template(&self) -> &Template;

    fn extension(&self) -> String;

    fn charge(&self) -> isize;

    fn write_input(&mut self, proc: Procedure);

    fn read_output(&self) -> Result<ProgramResult, ProgramError>;

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
