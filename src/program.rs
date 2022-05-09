pub mod mopac;

#[derive(Debug, PartialEq)]
pub enum ProgramStatus {
    Success(f64),
    FileNotFound,
    ErrorInOutput,
    EnergyNotFound,
    EnergyParseError,
}

pub trait Program {
    fn filename(&self) -> String;

    fn set_filename(&mut self, filename: &str);

    fn extension(&self) -> String;

    fn charge(&self) -> isize;

    fn write_input(&mut self);

    fn read_output(&self) -> ProgramStatus;

    /// Return all the filenames associated with the Program
    fn associated_files(&self) -> Vec<String>;
}

// TODO move this into a separate file
#[derive(Debug, Clone)]
pub struct Job<P: Program> {
    pub program: P,
    pub pbs_file: String,
    pub job_id: String,
    pub index: usize,
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
