use std::{
    fs::{read_to_string, File},
    path::Path,
    sync::OnceLock,
};

use regex::Regex;
use symm::Atom;

use crate::{
    geom::{geom_string, Geom},
    program::Procedure,
};

use super::{Program, ProgramError, ProgramResult, Template};

static INPUT_CELL: OnceLock<[Regex; 3]> = OnceLock::new();
static CELL: OnceLock<[Regex; 4]> = OnceLock::new();

struct DFTBPlus {
    filename: String,
    template: Template,
    charge: isize,
    geom: Geom,
}

impl Program for DFTBPlus {
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

    /// every file has to have the same name, so I don't actually need to match
    /// up extensions
    fn extension(&self) -> String {
        String::new()
    }

    fn charge(&self) -> isize {
        self.charge
    }

    /// Example [Template]:
    /// ```text
    /// Geometry = xyzFormat {
    /// {{.geom}}
    /// }
    ///
    /// Hamiltonian = DFTB {
    ///   Scc = Yes
    ///   SlaterKosterFiles = Type2FileNames {
    ///     Prefix = "/opt/dftb+/slako/mio/mio-1-1/"
    ///     Separator = "-"
    ///     Suffix = ".skf"
    ///   }
    ///   MaxAngularMomentum {
    ///     O = "p"
    ///     H = "s"
    ///   }
    ///   Charge = {{.charge}}
    /// }
    ///
    /// Options {}
    ///
    /// Analysis {
    ///   CalculateForces = Yes
    /// }
    ///
    /// ParserOptions {
    ///   ParserVersion = 12
    /// }

    /// ```
    fn write_input(&mut self, proc: Procedure) {
        use std::io::Write;
        let mut body = self.template().clone().header;
        // skip optgrad but accept optg at the end of a line
        let [opt, charge, geom_re] = INPUT_CELL.get_or_init(|| {
            [
                Regex::new(r"(?i)Driver = GeometryOptimization").unwrap(),
                Regex::new(r"\{\{.charge\}\}").unwrap(),
                Regex::new(r"\{\{.geom\}\}").unwrap(),
            ]
        });
        let mut found_opt = false;
        if opt.is_match(&body) {
            found_opt = true;
        }
        {
            use std::fmt::Write;
            match proc {
                Procedure::Opt => {
                    if !found_opt {
                        writeln!(
                            body,
                            r#"Driver = GeometryOptimization {{
  Optimizer = Rational {{}}
  MovedAtoms = 1:-1
  MaxSteps = 100
  OutputPrefix = "geom.out"
  Convergence {{GradAMax = 1E-8}}
}}"#,
                        )
                        .unwrap();
                    }
                }
                Procedure::Freq => todo!(),
                Procedure::SinglePt => {
                    if found_opt {
                        todo!("rewrite single point file without opt");
                    }
                }
            }
        }
        let geom = geom_string(&self.geom);
        let geom = if let Geom::Zmat(_) = &self.geom {
            panic!("don't know how to handle a Z-matrix in dftb+");
        } else {
            format!("{geom}\n")
        };
        body = geom_re.replace(&body, geom).to_string();
        body = charge
            .replace(&body, &format!("{}", self.charge))
            .to_string();

        let mut file = File::create(&self.filename).unwrap_or_else(|e| {
            panic!("failed to create {} with {e}", self.filename)
        });
        write!(file, "{body}").expect("failed to write input file");
    }

    fn read_output(filename: &str) -> Result<ProgramResult, ProgramError> {
        let path = Path::new(filename);

        // because the input file always has to have the same name, we know
        // there is a parent directory
        let parent = path.parent().unwrap();

        let outfile = parent.join("out");
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
                Regex::new(r"^Total\s+=\s+").unwrap(),
                Regex::new(r"^Total Energy: ").unwrap(),
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
        for line in contents.lines() {
            if time_re.is_match(line) {
                time = Some(
                    line.split_ascii_whitespace()
                        .nth(4)
                        .unwrap()
                        .parse()
                        .unwrap_or_else(|e| panic!("{e:#?}")),
                );
            } else if energy_re.is_match(line) {
                let energy_str = line.split_whitespace().nth(2);
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
        let geomfile = parent.join("geom.out.xyz");
        let cart_geom = if let Ok(s) = std::fs::read_to_string(&geomfile) {
            // always a proper XYZ file, so skip n atoms and comment lines
            let mut atoms = Vec::new();
            for line in s.lines().skip(2) {
                let mut sp = line.split_ascii_whitespace();
                // lines look like this with the charge(?) at the end, so take
                // the first four fields:
                // O   0.00000000  -0.71603315   0.00000000    6.59260702
                atoms.push(Atom::new_from_label(
                    sp.next().unwrap(),
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
        vec![
            "charges.bin".to_owned(),
            "detailed.out".to_owned(),
            "geom.out.gen".to_owned(),
            "geom.out.xyz".to_owned(),
            "band.out".to_owned(),
            "dftb_pin.hsd".to_owned(),
            "dftb_in.hsd".to_owned(),
        ]
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

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;
    use std::str::FromStr;

    use symm::Atom;

    use super::*;

    use crate::check;
    use crate::program::ProgramResult;

    #[test]
    fn write_input() {
        let template = Template::from(
            "
Geometry = xyzFormat {
{{.geom}}
}

Hamiltonian = DFTB {
  Scc = Yes
  SlaterKosterFiles = Type2FileNames {
    Prefix = \"/opt/dftb+/slako/mio/mio-1-1/\"
    Separator = \"-\"
    Suffix = \".skf\"
  }
  MaxAngularMomentum {
    O = \"p\"
    H = \"s\"
  }
  Charge = {{.charge}}
}

Options {
}

Analysis {
  CalculateForces = Yes
}

ParserOptions {
  ParserVersion = 12
}
",
        );

        let mut d = DFTBPlus {
            filename: "/tmp/dftb_in.hsd".into(),
            template,
            charge: 0,
            geom: Geom::from_str(
                "    3
Geometry Step: 9
    O      0.00000000     -0.71603315      0.00000000
    H      0.00000000     -0.14200298      0.77844804
    H     -0.00000000     -0.14200298     -0.77844804
",
            )
            .unwrap(),
        };

        d.write_input(Procedure::Opt);
        check!("testfiles/dftb+/single_opt.want", "/tmp/dftb_in.hsd");

        d.write_input(Procedure::SinglePt);
        check!("testfiles/dftb+/single_single.want", "/tmp/dftb_in.hsd");
    }

    #[test]
    fn read_opt_output() {
        let got = DFTBPlus::read_output("testfiles/dftb+/opt/out").unwrap();
        let want = ProgramResult {
            energy: -4.0779379326,
            cart_geom: Some(vec![
                Atom::new_from_label("O", 0.00000000, -0.71603315, 0.00000000),
                Atom::new_from_label("H", 0.00000000, -0.14200298, 0.77844804),
                Atom::new_from_label(
                    "H",
                    -0.00000000,
                    -0.14200298,
                    -0.77844804,
                ),
            ]),
            time: 0.05,
        };
        assert_eq!(got, want);
    }
}
