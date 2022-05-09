pub mod mopac;

#[derive(Debug, PartialEq)]
pub enum ProgramStatus {
    Success(f64),
    FileNotFound,
    ErrorInOutput,
    EnergyNotFound,
    EnergyParseError,
}

#[derive(Debug, PartialEq)]
pub enum Procedure {
    Opt,
    Freq,
    SinglePt,
}

pub trait Program {
    fn filename(&self) -> String;

    fn set_filename(&mut self, filename: &str);

    fn extension(&self) -> String;

    fn charge(&self) -> isize;

    fn write_input(&mut self, proc: Procedure);

    fn read_output(&self, proc: Procedure) -> ProgramStatus;

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
