use crate::geom::{geom_string, Geom};
use crate::program::{Program, ProgramError};
use lazy_static::lazy_static;
use regex::Regex;
use symm::Atom;

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
    pub param_file: String,
    pub param_dir: String,
    pub charge: isize,
    pub template: Template,
}

impl Program for Mopac {
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
            self.param_file = format!("{}/{}", self.param_dir, s.finish());
            Self::write_params(params, &self.param_file);
            write!(header, " external={}", self.param_file).unwrap();
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
        let contents = match read_to_string(outfile) {
            Ok(s) => s,
            Err(_) => {
                return Err(ProgramError::FileNotFound);
            } // file not found
        };
        lazy_static! {
            static ref PANIC: Regex = Regex::new("(?i)panic").unwrap();
            static ref ERROR: Regex = Regex::new("(?i)error").unwrap();
            static ref DONE: Regex = Regex::new(" == MOPAC DONE ==").unwrap();
        }
        if PANIC.is_match(&contents) {
            eprintln!("panic requested in read_output");
            std::process::exit(1)
        } else if ERROR.is_match(&contents) {
            return Err(ProgramError::ErrorInOutput);
        } else if DONE.is_match(&contents) {
            return self.read_aux();
        }
        Err(ProgramError::EnergyNotFound)
    }

    fn associated_files(&self) -> Vec<String> {
        let fname = self.filename();
        vec![
            format!("{}.mop", fname),
            format!("{}.out", fname),
            format!("{}.arc", fname),
            format!("{}.aux", fname),
            self.param_file.clone(),
        ]
    }

    fn charge(&self) -> isize {
        self.charge
    }
}

impl Mopac {
    pub fn new(
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
            param_file: String::new(),
            param_dir: "tmparam".to_string(),
            charge,
            template,
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
                Mopac::new(
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
            return Err(ProgramError::FileNotFound);
        };
        let lines = BufReader::new(f).lines().flatten();
        let mut res = ProgramResult {
            energy: 0.0,
            cart_geom: Vec::new(),
        };
        lazy_static! {
            static ref HEAT: Regex = Regex::new("HEAT_OF_FORMATION").unwrap();
            static ref ATOM: Regex = Regex::new("ATOM_X_OPT").unwrap();
            static ref ELEMENT: Regex = Regex::new("ATOM_EL").unwrap();
            static ref CHARGE: Regex = Regex::new("ATOM_CHARGES").unwrap();
        }
        let mut ok = false;
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
                        res.energy = f / KCALHT;
                        ok = true;
                    }
                    Err(_) => {
                        return Err(ProgramError::EnergyParseError);
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
                ok = true;
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
        for (c, coord) in coords.iter().enumerate() {
            res.cart_geom.push(Atom::new_from_label(
                &labels[c], coord[0], coord[1], coord[2],
            ));
        }
        if ok {
            Ok(res)
        } else {
            Err(ProgramError::EnergyNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs::{self, read_to_string};

    use crate::program::Procedure;
    use crate::string;

    use crate::queue::{self, Queue};

    use super::*;

    fn test_mopac() -> Mopac {
        let names = vec![
            "USS", "ZS", "BETAS", "GSS", "USS", "UPP", "ZS", "ZP", "BETAS",
            "BETAP", "GSS", "GPP", "GSP", "GP2", "HSP",
        ];
        let atoms = vec![
            "H", "H", "H", "H", "C", "C", "C", "C", "C", "C", "C", "C", "C",
            "C", "C",
        ];
        #[rustfmt::skip]
        let values = vec![
            -11.246958000000, 1.268641000000, -8.352984000000,
            14.448686000000, -51.089653000000, -39.937920000000,
            2.047558000000, 1.702841000000, -15.385236000000,
            -7.471929000000, 13.335519000000, 10.778326000000,
            11.528134000000, 9.486212000000, 0.717322000000,
        ];
        Mopac::new(
            String::from("/tmp/test"),
            Some(Rc::new(Params::from(
                names.iter().map(|s| s.to_string()).collect(),
                atoms.iter().map(|s| s.to_string()).collect(),
                values,
            ))),
            Rc::new(Geom::Xyz(Vec::new())),
            0,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        )
    }

    #[test]
    fn test_write_input() {
        let mut tm = Mopac {
            params: None,
            ..test_mopac()
        };
        tm.param_dir = "/tmp".to_string();
        tm.write_input(Procedure::SinglePt);
        let got = fs::read_to_string("/tmp/test.mop").expect("file not found");
        let want = "scfcrt=1.D-21 aux(precision=14) PM6 A0 charge=0 1SCF XYZ
Comment line 1
Comment line 2

"
        .to_string();
        assert_eq!(got, want);
        fs::remove_file("/tmp/test.mop").unwrap();
    }

    #[test]
    fn test_write_input_with_params() {
        let mut tm = test_mopac();
        tm.param_dir = "/tmp".to_string();
        tm.write_input(Procedure::SinglePt);
        let got = fs::read_to_string("/tmp/test.mop").expect("file not found");
        let want = format!(
            "scfcrt=1.D-21 aux(precision=14) PM6 A0 charge=0 1SCF \
	     external={} XYZ
Comment line 1
Comment line 2

",
            tm.param_file,
        );
        assert_eq!(got, want);
        fs::remove_file("/tmp/test.mop").unwrap();
    }

    #[test]
    fn test_write_params() {
        let tm = test_mopac();
        Mopac::write_params(
            &tm.params.unwrap(),
            &String::from("/tmp/params.dat"),
        );
        let got =
            fs::read_to_string("/tmp/params.dat").expect("file not found");
        let want = "USS H -11.246958000000
ZS H 1.268641000000
BETAS H -8.352984000000
GSS H 14.448686000000
USS C -51.089653000000
UPP C -39.937920000000
ZS C 2.047558000000
ZP C 1.702841000000
BETAS C -15.385236000000
BETAP C -7.471929000000
GSS C 13.335519000000
GPP C 10.778326000000
GSP C 11.528134000000
GP2 C 9.486212000000
HSP C 0.717322000000
";
        assert_eq!(got, want);
        fs::remove_file("/tmp/params.dat").unwrap();
    }

    extern crate test;
    use test::Bencher;
    #[bench]
    fn bench_read_output(b: &mut Bencher) {
        // success
        let mp = Mopac::new(
            String::from("testfiles/job"),
            None,
            Rc::new(Geom::Xyz(Vec::new())),
            0,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        );
        b.iter(|| mp.read_output());
    }

    #[test]
    fn test_read_output() {
        // success
        let mp = Mopac::new(
            String::from("testfiles/job"),
            None,
            Rc::new(Geom::Xyz(Vec::new())),
            0,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        );
        let got = mp.read_output().unwrap().energy;
        let want = 9.712_794_745_916_472e1 / KCALHT;
        assert!((got - want).abs() < 1e-20);

        // opt success
        let mp = Mopac::new(
            String::from("testfiles/opt"),
            None,
            Rc::new(Geom::Xyz(Vec::new())),
            1,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        );
        let got = mp.read_output().unwrap().cart_geom;
        let want = vec![
            Atom::new_from_label(
                "C",
                0.000000000000000000,
                0.000000000000000000,
                0.000000000000000000,
            ),
            Atom::new_from_label(
                "C",
                1.436_199_643_883_821_2,
                0.000000000000000000,
                0.000000000000000000,
            ),
            Atom::new_from_label(
                "C",
                0.799_331_622_330_450_3,
                1.193_205_084_901_411_7,
                0.000000000000000000,
            ),
            Atom::new_from_label(
                "H",
                2.360_710_453_618_393,
                -0.506_038_360_297_709_7,
                0.000000000000026804,
            ),
            Atom::new_from_label(
                "H",
                0.893_457_241_509_136_9,
                2.242_936_206_295_408_6,
                -0.000000000000026804,
            ),
        ];
        assert_eq!(got, want);

        // failure in output
        let mp = Mopac::new(
            String::from("testfiles/nojob"),
            None,
            Rc::new(Geom::Xyz(Vec::new())),
            0,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        );
        let got = mp.read_output();
        assert_eq!(got.err().unwrap(), ProgramError::EnergyNotFound);

        // failure in aux
        let mp = Mopac::new(
            String::from("testfiles/noaux"),
            None,
            Rc::new(Geom::Xyz(Vec::new())),
            0,
            Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        );
        let got = mp.read_output();
        assert_eq!(got.err().unwrap(), ProgramError::FileNotFound);
    }

    /// minimal queue for testing general submission
    struct TestQueue;

    impl Queue<Mopac> for TestQueue {
        fn write_submit_script(&self, infiles: &[String], filename: &str) {
            let mut body = String::new();
            for f in infiles {
                body.push_str(&format!("echo {f}\n"));
            }
            let mut file =
                File::create(filename).expect("failed to create params file");
            write!(file, "{}", body).expect("failed to write params file");
        }

        fn submit_command(&self) -> &str {
            "bash"
        }

        fn chunk_size(&self) -> usize {
            128
        }

        fn job_limit(&self) -> usize {
            1600
        }

        fn sleep_int(&self) -> usize {
            1
        }

        const SCRIPT_EXT: &'static str = "pbs";

        fn dir(&self) -> &str {
            "inp"
        }

        fn stat_cmd(&self) -> String {
            todo!()
        }

        fn status(&self) -> HashSet<String> {
            todo!()
        }
    }

    #[test]
    fn test_submit() {
        let tq = TestQueue;
        tq.write_submit_script(
            &string!["input1.mop", "input2.mop", "input3.mop"],
            "/tmp/main.pbs",
        );
        let got = tq.submit("/tmp/main.pbs");
        let want = "input3.mop";
        assert_eq!(got, want);
    }

    #[test]
    fn test_resubmit() {
        use std::path::Path;
        let tq = TestQueue;
        std::fs::copy("testfiles/job.mop", "/tmp/job.mop").unwrap();
        let got = tq.resubmit("/tmp/job.mop");
        assert!(Path::new("/tmp/job_redo.mop").exists());
        assert!(Path::new("/tmp/job_redo.pbs").exists());
        assert_eq!(
            read_to_string("/tmp/job.mop").unwrap(),
            read_to_string("/tmp/job_redo.mop").unwrap()
        );
        let want = queue::Resubmit {
            inp_file: String::from("/tmp/job_redo"),
            pbs_file: String::from("/tmp/job_redo.pbs"),
            job_id: String::from("/tmp/job_redo"),
        };
        assert_eq!(got, want);

        for f in ["/tmp/job.mop", "/tmp/job_redo.mop", "/tmp/job_redo.pbs"] {
            std::fs::remove_file(f).unwrap();
        }
    }
}
