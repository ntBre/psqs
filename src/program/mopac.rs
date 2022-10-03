use crate::geom::{geom_string, Geom};
use crate::program::{Program, ProgramError};
use lazy_static::lazy_static;
use regex::Regex;
use symm::Atom;

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::fs::{read_to_string, File};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::rc::Rc;

use super::{Job, Procedure, ProgramResult, Template};

/// kcal/mol per hartree
pub const KCALHT: f64 = 627.5091809;

pub use self::params::*;
pub mod params;

#[cfg(test)]
mod tests;

/// Mopac holds the information needed to write a MOPAC input file. `filename`
/// should not include an extension. `.mop` will be appended for input files,
/// and `.out` and `.aux` will be appended for output files.
#[derive(Debug, Clone)]
pub struct Mopac {
    pub filename: String,

    /// The semi-empirical parameters to use in the calculation via the EXTERNAL
    /// keyword. These are wrapped in an Rc to allow the same set of parameters
    /// to be shared between calculations without an expensive `clone`
    /// operation.
    pub params: Option<Rc<Params>>,

    /// The initial geometry for the calculation. These are also wrapped in an
    /// Rc to avoid allocating multiple copies for calculations with the same
    /// geometry.
    pub geom: Rc<Geom>,

    /// the file in which to store the parameters
    pub param_file: Option<String>,

    /// the directory in which to write `param_file`. TODO this option and the
    /// params option should be combined. param_file is set in write_params, so
    /// it's genuinely separate: initially None and then Some after that
    pub param_dir: Option<String>,

    /// molecular charge, included in the input file via the CHARGE keyword
    pub charge: isize,

    /// [Template] for the input file
    pub template: Template,

    last_hash: RefCell<u64>,
}

impl Program for Mopac {
    fn new(
        filename: String,
        template: Template,
        charge: isize,
        geom: Geom,
    ) -> Self {
        Self {
            filename,
            geom: Rc::new(geom),
            param_file: None,
            charge,
            template,
            params: None,
            param_dir: None,
            last_hash: RefCell::new(0),
        }
    }

    fn filename(&self) -> String {
        self.filename.clone()
    }

    fn set_filename(&mut self, filename: &str) {
        self.filename = String::from(filename);
    }

    fn template(&self) -> &Template {
        &self.template
    }

    fn extension(&self) -> String {
        String::from("mop")
    }

    /// Writes the parameters of self to a parameter file, then writes the MOPAC
    /// input file with external=paramfile. Also update self.paramfile to point
    /// to the generated name for the parameter file
    fn write_input(&mut self, proc: Procedure) {
        use std::fmt::Write;
        // header should look like
        //   scfcrt=1.D-21 aux(precision=14) PM6
        // so that the charge, and optionally XYZ, A0, and 1SCF can be added
        let mut header = self.template().clone().header;
        write!(header, " charge={}", self.charge).unwrap();
        match proc {
            Procedure::Opt => {
                // optimization is the default, so just don't add 1SCF
            }
            Procedure::Freq => todo!(),
            Procedure::SinglePt => {
                header.push_str(" 1SCF");
            }
        }
        if let Some(params) = &self.params {
            let mut s = DefaultHasher::new();
            self.filename.hash(&mut s);
            let param_file =
                format!("{}/{}", self.param_dir.as_ref().unwrap(), s.finish());
            Self::write_params(params, &param_file);
            write!(header, " external={}", param_file).unwrap();
            self.param_file = Some(param_file);
        }
        if self.geom.is_xyz() {
            header.push_str(" XYZ");
        }
        let geom = geom_string(&self.geom);
        let filename = format!("{}.mop", self.filename);
        let mut file = match File::create(&filename) {
            Ok(f) => f,
            Err(e) => panic!("failed to create {filename} with {e}"),
        };
        write!(
            file,
            "{header}
Comment line 1
Comment line 2
{geom}
",
        )
        .expect("failed to write input file");
    }

    /// Reads a MOPAC output file. If normal termination occurs, also try
    /// reading the `.aux` file to extract the energy from there. This function
    /// panics if an error is found in the output file. If a non-fatal error
    /// occurs (file not found, not written to yet, etc) None is returned.
    fn read_output(&self) -> Result<ProgramResult, ProgramError> {
        let outfile = format!("{}.out", &self.filename);
        let contents = match read_to_string(&outfile) {
            Ok(s) => s,
            Err(_) => {
                return Err(ProgramError::FileNotFound(outfile));
            }
        };
        let mut hasher = DefaultHasher::new();
        contents.hash(&mut hasher);
        let hash = hasher.finish();
        let mut s = self.last_hash.borrow_mut();
        // file hasn't changed
        if *s == hash {
            return Err(ProgramError::EnergyNotFound(outfile));
        }
        *s = hash;
        lazy_static! {
            static ref PANIC: Regex = Regex::new("(?i)panic").unwrap();
            static ref ERROR: Regex = Regex::new("(?i)error").unwrap();
            static ref DONE: Regex = Regex::new(" == MOPAC DONE ==").unwrap();
        }
        if DONE.is_match(&contents) {
            return self.read_aux();
        } else if ERROR.is_match(&contents) {
            return Err(ProgramError::ErrorInOutput(self.filename.clone()));
        } else if PANIC.is_match(&contents) {
            panic!("panic requested in read_output");
        }
        Err(ProgramError::EnergyNotFound(outfile))
    }

    fn associated_files(&self) -> Vec<String> {
        let fname = self.filename();
        let mut ret = vec![
            format!("{}.mop", fname),
            format!("{}.out", fname),
            format!("{}.arc", fname),
            format!("{}.aux", fname),
        ];
        if let Some(f) = self.param_file.clone() {
            ret.push(f);
        }
        ret
    }

    fn charge(&self) -> isize {
        self.charge
    }
}

impl Mopac {
    pub fn new_full(
        filename: String,
        params: Option<Rc<Params>>,
        geom: Rc<Geom>,
        charge: isize,
        template: Template,
    ) -> Self {
        Self {
            filename,
            params,
            geom,
            param_file: None,
            param_dir: Some("tmparam".to_string()),
            charge,
            template,
            last_hash: RefCell::new(0),
        }
    }

    /// Build the jobs described by `moles` in memory, but don't write any of their
    /// files yet
    #[allow(clippy::too_many_arguments)]
    pub fn build_jobs(
        moles: &Vec<Rc<Geom>>,
        params: Option<&Params>,
        dir: &'static str,
        start_index: usize,
        coeff: f64,
        job_num: usize,
        charge: isize,
        tmpl: Template,
    ) -> Vec<Job<Mopac>> {
        let mut count: usize = start_index;
        let mut job_num = job_num;
        let mut jobs = Vec::new();
        let params = params.map(|p| Rc::new(p.clone()));
        for mol in moles {
            let filename = format!("{dir}/job.{:08}", job_num);
            job_num += 1;
            let mut job = Job::new(
                Mopac::new_full(
                    filename,
                    params.clone(),
                    mol.clone(),
                    charge,
                    tmpl.clone(),
                ),
                count,
            );
            job.coeff = coeff;
            jobs.push(job);
            count += 1;
        }
        jobs
    }

    fn write_params(params: &Rc<Params>, filename: &str) {
        let body = params.to_string();
        let mut file = match File::create(filename) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to create {} with {}", filename, e);
                std::process::exit(1);
            }
        };
        write!(file, "{}", body).expect("failed to write params file");
    }

    /// return the heat of formation from a MOPAC aux file in Hartrees
    fn read_aux(&self) -> Result<ProgramResult, ProgramError> {
        let auxfile = format!("{}.aux", &self.filename);
        let f = if let Ok(file) = File::open(&auxfile) {
            file
        } else {
            return Err(ProgramError::FileNotFound(auxfile));
        };
        let lines = BufReader::new(f).lines().flatten();
        let mut energy = None;
        lazy_static! {
            static ref HEAT: Regex = Regex::new("HEAT_OF_FORMATION").unwrap();
            static ref ATOM: Regex = Regex::new("ATOM_X_OPT").unwrap();
            static ref ELEMENT: Regex = Regex::new("ATOM_EL").unwrap();
            static ref CHARGE: Regex = Regex::new("ATOM_CHARGES").unwrap();
        }
        #[derive(PartialEq)]
        enum State {
            Geom,
            Labels,
            None,
        }
        struct Guard {
            heat: bool,
            atom: bool,
            element: bool,
        }
        let mut state = State::None;
        let mut guard = Guard {
            heat: false,
            atom: false,
            element: false,
        };
        // atomic labels
        let mut labels = Vec::new();
        // coordinates
        let mut coords: Vec<Vec<f64>> = Vec::new();
        for line in lines {
            // line like HEAT_OF_FORMATION:KCAL/MOL=+0.97127947459164715838D+02
            if !guard.heat && HEAT.is_match(&line) {
                let fields: Vec<&str> = line.trim().split('=').collect();
                match fields[1].replace('D', "E").parse::<f64>() {
                    Ok(f) => {
                        energy = Some(f / KCALHT);
                    }
                    Err(_) => {
                        return Err(ProgramError::EnergyParseError(auxfile));
                    }
                }
                guard.heat = true;
            } else if !guard.atom && ATOM.is_match(&line) {
                state = State::Geom;
                guard.atom = true;
            } else if state == State::Geom && CHARGE.is_match(&line) {
                break;
            } else if state == State::Geom {
                coords.push(
                    line.split_whitespace()
                        .map(|s| s.parse().unwrap())
                        .collect(),
                );
            } else if !guard.element && ELEMENT.is_match(&line) {
                state = State::Labels;
                guard.element = true;
            } else if state == State::Labels {
                labels = line
                    .split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                state = State::None;
            }
        }
        let cart_geom = if coords.is_empty() {
            None
        } else {
            let mut ret = Vec::new();
            for (c, coord) in coords.iter().enumerate() {
                ret.push(Atom::new_from_label(
                    &labels[c], coord[0], coord[1], coord[2],
                ));
            }
            Some(ret)
        };
        if let Some(energy) = energy {
            Ok(ProgramResult { energy, cart_geom })
        } else {
            Err(ProgramError::EnergyNotFound(auxfile))
        }
    }
}
