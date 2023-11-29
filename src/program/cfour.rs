use std::{
    fs::{read_to_string, File},
    path::Path,
    sync::OnceLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use symm::Atom;

use crate::{
    geom::Geom,
    queue::{local::Local, pbs::Pbs, slurm::Slurm, Queue, Submit},
};

use super::{Procedure, Program, ProgramError, ProgramResult, Template};

#[derive(Clone, Deserialize, Serialize)]
pub struct Cfour {
    /// in this case, `filename` is actually a directory name because every
    /// CFOUR input file has to have the same name (ZMAT)
    filename: String,
    template: Template,
    charge: isize,
    geom: Geom,
}

static INPUT_CELL: OnceLock<[Regex; 3]> = OnceLock::new();
static CELL: OnceLock<[Regex; 4]> = OnceLock::new();

impl Program for Cfour {
    fn filename(&self) -> String {
        self.filename.clone()
    }

    fn infile(&self) -> String {
        todo!()
    }

    fn set_filename(&mut self, filename: &str) {
        self.filename = filename.into();
    }

    fn template(&self) -> &Template {
        &self.template
    }

    /// ZMAT has no extension
    fn extension(&self) -> String {
        String::new()
    }

    fn charge(&self) -> isize {
        self.charge
    }

    /// Example [Template]:
    /// ```text
    /// comment line
    /// {{.geom}}
    ///
    /// *CFOUR(CALC=CCSD,BASIS=PVTZ,MEMORY_SIZE=8,MEM_UNIT=GB,REF=RHF,MULT=1
    /// {{.keywords}})
    /// ```
    fn write_input(&mut self, proc: Procedure) {
        use std::io::Write;
        let mut body = self.template().clone().header;
        // always just paste in the geometry, assume it's a zmat for
        // optimization and cartesian for single point
        body = body.replace("{{.geom}}", &self.geom.to_string());
        match proc {
            Procedure::Opt => {
                if !self.geom.is_zmat() {
                    panic!("CFOUR requires Z-matrix for optimization");
                }
            }
            Procedure::SinglePt => {
                if !self.geom.is_xyz() {
                    panic!(
                        "pbqff expects Cartesian geometry for single-points"
                    );
                }
                // default units are Angstrom
                body = body.replace("{{.keywords}}", "COORD=CARTESIAN");
            }
            Procedure::Freq => todo!(),
        };
        let dir = Path::new(&self.filename);
        std::fs::create_dir_all(dir).unwrap_or_else(|e| {
            panic!("failed to create {} with {e}", self.filename)
        });
        let mut file = File::create(dir.join("ZMAT")).unwrap_or_else(|e| {
            panic!("failed to create dftb input in {} with {e}", self.filename)
        });
        write!(file, "{body}").expect("failed to write input file");
    }

    fn read_output(filename: &str) -> Result<ProgramResult, ProgramError> {
        let path = Path::new(filename);

        let outfile = path.join("output.dat");
        let outname = outfile.to_string_lossy().to_string();
        let contents = match read_to_string(&outfile) {
            Ok(s) => s,
            Err(_) => {
                return Err(ProgramError::FileNotFound(outname));
            }
        };

        let [panic_re, error_re, time_re, energy_re] = CELL.get_or_init(|| {
            [
                Regex::new("(?i)panic").unwrap(),
                Regex::new(r"\bERROR\b").unwrap(),
                Regex::new(r"--Timing info--").unwrap(),
                Regex::new(r"The final electronic energy is").unwrap(),
            ]
        });

        if panic_re.is_match(&contents) {
            panic!("panic requested in read_output");
        } else if error_re.is_match(&contents) {
            return Err(ProgramError::ErrorInOutput(outname));
        }

        // main output
        let mut energy = None;
        let mut time = None;
        let mut next_time = false;
        for line in contents.lines() {
            if time_re.is_match(line) {
                next_time = true;
            } else if next_time {
                next_time = false;
                // parse a line like:
                // 127.194u 55.263s 3:10.96 95.5% 0+0k 280+1319824io 1pf+0w
                time = Some(
                    line.split_ascii_whitespace()
                        .nth(1)
                        .unwrap()
                        .trim_end_matches('s')
                        .parse()
                        .unwrap(),
                );
            } else if energy_re.is_match(line) {
                let energy_str = line.split_whitespace().nth(5);
                if let Some(e) = energy_str {
                    energy = if let Ok(v) = e.parse::<f64>() {
                        Some(v)
                    } else {
                        return Err(ProgramError::EnergyParseError(outname));
                    }
                } else {
                    return Err(ProgramError::EnergyParseError(outname));
                }
            }
        }

        // read xyz. TODO we only need to do this if it's an optimization
        let geomfile = path.join("MOLDEN");
        let cart_geom = if let Ok(s) = std::fs::read_to_string(geomfile) {
            // skip [Molden Format] and [ATOMS] lines
            let mut atoms = Vec::new();
            for line in s.lines().skip(2) {
                if line.starts_with("[Molden Format]") {
                    break;
                }
                let mut sp = line.split_ascii_whitespace();
                atoms.push(Atom::new(
                    sp.nth(2).unwrap().parse().unwrap(),
                    sp.next().unwrap().parse().unwrap(),
                    sp.next().unwrap().parse().unwrap(),
                    sp.next().unwrap().parse().unwrap(),
                ));
            }
            Some(atoms)
        } else {
            None
        };

        let Some(energy) = energy else {
            return Err(ProgramError::EnergyNotFound(outname));
        };

        let Some(time) = time else {
            // the time is the last thing printed, so don't trust the energy if
            // we don't find the time. we could have read an earlier energy in a
            // geometry optimization, for example
            return Err(ProgramError::EnergyNotFound(outname));
        };

        Ok(ProgramResult {
            energy,
            cart_geom,
            time,
        })
    }

    fn associated_files(&self) -> Vec<String> {
        [
            "ECPDATA",
            "GENBAS",
            "output.dat",
            "ZMATnew",
            "FCMINT",
            "MOLDEN",
            "ZMAT",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn new(
        filename: String,
        template: Template,
        charge: isize,
        geom: Geom,
    ) -> Self {
        Self {
            filename,
            template,
            charge,
            geom,
        }
    }
}

impl Submit<Cfour> for Pbs {}

impl Queue<Cfour> for Pbs {
    fn default_submit_script(&self) -> String {
        "#!/bin/sh
#PBS -N {{.basename}}
#PBS -S /bin/bash
#PBS -j oe
#PBS -o {{.filename}}.out
#PBS -W umask=022
#PBS -l walltime=1000:00:00
#PBS -l ncpus=1
#PBS -l mem=8gb
#PBS -q workq

module load openpbs

export WORKDIR=$PBS_O_WORKDIR
cd $WORKDIR

CFOUR_SCRIPT=/ddn/home8/r2610/bin/c4ext_new.sh
"
        .to_owned()
    }

    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        use std::fmt::Write;
        let path = Path::new(filename);
        let basename = path.file_name().unwrap();
        let mut body = self
            .template
            .clone()
            .unwrap_or_else(|| {
                <Self as Queue<Cfour>>::default_submit_script(self)
            })
            .replace("{{.basename}}", basename.to_str().unwrap())
            .replace("{{.filename}}", filename);
        for f in infiles {
            writeln!(body, "(cd {f} && $CFOUR_SCRIPT $NCPUS)").unwrap();
        }
        if std::fs::write(filename, body).is_err() {
            panic!("write_submit_script: failed to create {filename}");
        };
    }
}

impl Queue<Cfour> for Slurm {
    fn default_submit_script(&self) -> String {
        todo!()
    }

    fn write_submit_script(&self, _infiles: &[String], _filename: &str) {
        todo!()
    }
}

impl Submit<Cfour> for Local {}

impl Queue<Cfour> for Local {
    fn default_submit_script(&self) -> String {
        todo!()
    }

    fn write_submit_script(&self, _infiles: &[String], _filename: &str) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use symm::Atom;

    use crate::check;

    use super::*;

    #[test]
    fn read_output() {
        let got = Cfour::read_output("testfiles/cfour").unwrap();
        let want = ProgramResult {
            energy: -76.338010630480653,
            cart_geom: Some(vec![
                Atom::new(8, -0.0000000000, 0.0000000000, 0.1243564077),
                Atom::new(1, 0.0000000000, -1.4232661785, -0.9868132143),
                Atom::new(1, 0.0000000000, 1.4232661785, -0.9868132143),
            ]),
            time: 55.263,
        };
        assert_eq!(got, want);
    }

    #[test]
    fn write_input() {
        let template = Template::from(
            "comment line
{{.geom}}

*CFOUR(CALC=CCSD,BASIS=PVTZ,MEMORY_SIZE=8,MEM_UNIT=GB,REF=RHF,MULT=1
{{.keywords}})
",
        );

        let mut d = Cfour {
            filename: "/tmp".into(),
            template,
            charge: 0,
            geom: Geom::from_str(
                "
O        -0.000000000         0.000000000         0.065806577
H         0.000000000        -0.753160027        -0.522199064
H         0.000000000         0.753160027        -0.522199064
",
            )
            .unwrap(),
        };

        d.write_input(Procedure::SinglePt);
        check!("testfiles/cfour/ZMAT.want", "/tmp/ZMAT");
    }
}
