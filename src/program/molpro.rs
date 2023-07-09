use std::{
    cell::LazyCell,
    fs::{read_to_string, File},
};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::geom::{geom_string, Geom};

use super::{Procedure, Program, ProgramError, ProgramResult, Template};

#[cfg(test)]
mod tests;

#[derive(Clone, Serialize, Deserialize)]
pub struct Molpro {
    filename: String,
    template: Template,
    charge: isize,
    geom: Geom,
}

impl Program for Molpro {
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
        String::from("inp")
    }

    fn charge(&self) -> isize {
        self.charge
    }

    /// Example [Template]:
    /// ```text
    /// memory,1,g
    /// gthresh,energy=1.d-12,zero=1.d-22,oneint=1.d-22,twoint=1.d-22;
    /// gthresh,optgrad=1.d-8,optstep=1.d-8;
    /// nocompress;
    ///
    /// geometry={
    /// {{.geom}}
    /// ! note the missing closing brace!
    /// basis={
    /// default,cc-pVTZ-f12
    /// }
    /// set,charge={{.charge}}
    /// set,spin=0
    /// hf,accuracy=16,energy=1.0d-10
    /// {CCSD(T)-F12,thrden=1.0d-8,thrvar=1.0d-10}
    /// {optg,grms=1.d-8,srms=1.d-8}
    /// ```
    ///
    /// In line with [Go templates](https://pkg.go.dev/text/template),
    /// `{{.geom}}` is replaced with `self.geom`, and `{{.charge}}` is
    /// replaced with `self.charge`. If `proc` is `Procedure::Opt`, and the
    /// template includes this optg line, the line is left there. If the
    /// procedure is `Opt` and the line is absent, it will be added.
    /// Similarly, if `proc` is not `Opt` and the line is present in the
    /// template, it will be deleted.
    ///
    /// The missing closing brace around the geometry allows for easier handling
    /// of ZMAT inputs since `write_input` can insert its own closing brace
    /// between the ZMAT and parameter values.
    fn write_input(&mut self, proc: Procedure) {
        use std::io::Write;
        let mut body = self.template().clone().header;
        // skip optgrad but accept optg at the end of a line
        let opt = LazyCell::new(|| Regex::new(r"(?i)optg(,|\s*$)").unwrap());
        let optg_line =
            LazyCell::new(|| Regex::new(r"(?i)^.*optg(,|\s*$)").unwrap());
        let charge = LazyCell::new(|| Regex::new(r"\{\{.charge\}\}").unwrap());
        let geom_re = LazyCell::new(|| Regex::new(r"\{\{.geom\}\}").unwrap());
        let mut found_opt = false;
        if opt.is_match(&body) {
            found_opt = true;
        }
        {
            use std::fmt::Write;
            match proc {
                Procedure::Opt => {
                    if !found_opt {
                        writeln!(body, "{{optg,grms=1.d-8,srms=1.d-8}}")
                            .unwrap();
                    }
                }
                Procedure::Freq => todo!(),
                Procedure::SinglePt => {
                    if found_opt {
                        let mut new = String::new();
                        for line in body.lines() {
                            if !optg_line.is_match(line) {
                                writeln!(new, "{line}").unwrap();
                            }
                        }
                        body = new;
                    }
                }
            }
        }
        let geom = geom_string(&self.geom);
        let geom = if let Geom::Zmat(_) = &self.geom {
            use std::fmt::Write;
            let mut new_lines = String::new();
            let mut found = false;
            for line in geom.lines() {
                if line.contains('=') && !found {
                    found = true;
                    new_lines.push_str("}\n");
                }
                writeln!(new_lines, "{line}").unwrap();
            }
            new_lines
        } else {
            format!("{geom}\n}}\n")
        };
        body = geom_re.replace(&body, geom).to_string();
        body = charge
            .replace(&body, &format!("{}", self.charge))
            .to_string();

        let filename = format!("{}.{}", self.filename, self.extension());
        let mut file = match File::create(&filename) {
            Ok(f) => f,
            Err(e) => panic!("failed to create {filename} with {e}"),
        };
        write!(file, "{body}").expect("failed to write input file");
    }

    fn read_output(filename: &str) -> Result<ProgramResult, ProgramError> {
        let outfile = format!("{}.out", &filename);
        let contents = match read_to_string(&outfile) {
            Ok(s) => s,
            Err(_) => {
                return Err(ProgramError::FileNotFound(outfile));
            }
        };
        let panic_re = LazyCell::new(|| Regex::new("(?i)panic").unwrap());
        let error_re = LazyCell::new(|| Regex::new(r"(?i)\berror\b").unwrap());
        let geom_re = LazyCell::new(|| Regex::new("Current geometry").unwrap());
        let blank_re = LazyCell::new(|| Regex::new(r"^\s*$").unwrap());
        let time_re = LazyCell::new(|| Regex::new(r"^ REAL TIME").unwrap());
        let energy_re = LazyCell::new(|| Regex::new(r"^ PBQFF\s+=").unwrap());

        if panic_re.is_match(&contents) {
            panic!("panic requested in read_output");
        } else if error_re.is_match(&contents) {
            return Err(ProgramError::ErrorInOutput(outfile));
        }

        let mut energy = None;
        let mut skip = 0;
        let mut geom = false;
        let mut atoms = Vec::new();
        let mut time = 0.0;
        for line in contents.lines() {
            if skip > 0 {
                skip -= 1;
            } else if time_re.is_match(line) {
                time = line
                    .split_ascii_whitespace()
                    .nth(3)
                    .unwrap()
                    .parse()
                    .unwrap_or_else(|e| panic!("{e:#?}"));
            } else if energy_re.is_match(line) {
                let energy_str = line.split_whitespace().nth(2);
                if let Some(e) = energy_str {
                    energy = if let Ok(v) = e.parse::<f64>() {
                        Some(v)
                    } else {
                        return Err(ProgramError::EnergyParseError(outfile));
                    }
                } else {
                    return Err(ProgramError::EnergyParseError(outfile));
                }
            } else if geom_re.is_match(line) {
                skip = 3;
                geom = true;
            } else if geom && blank_re.is_match(line) {
                geom = false;
            } else if geom {
                let sp: Vec<_> = line.split_whitespace().collect();
                // kinda sad to panic here, but not sure what else to do. could
                // return a GeomParse error, but then that's irrelevant to a
                // caller who only wants the energy. maybe we just set geom to
                // false and reset atoms to be empty
                atoms.push(symm::Atom::new_from_label(
                    sp[0],
                    sp[1].parse().unwrap(),
                    sp[2].parse().unwrap(),
                    sp[3].parse().unwrap(),
                ));
            }
        }

        if let Some(energy) = energy {
            return Ok(ProgramResult {
                energy,
                cart_geom: if atoms.is_empty() { None } else { Some(atoms) },
                time,
            });
        }

        Err(ProgramError::EnergyNotFound(outfile))
    }

    fn associated_files(&self) -> Vec<String> {
        vec![self.infile(), self.outfile()]
    }

    fn infile(&self) -> String {
        self.filename() + ".inp"
    }
}
