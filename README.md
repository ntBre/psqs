[![check](https://github.com/ntBre/psqs/actions/workflows/check.yml/badge.svg)](https://github.com/ntBre/psqs/actions/workflows/check.yml)
[![test](https://github.com/ntBre/psqs/actions/workflows/test.yml/badge.svg)](https://github.com/ntBre/psqs/actions/workflows/test.yml)

# psqs

quantum chemistry ProgramS and Queuing systemS

# Description

This is a crate for Rust interfaces to quantum chemistry programs (like Molpro,
Gaussian, and MOPAC) and queuing systems like Slurm and PBS. It contains traits
defining the high-level behavior of these `Program`s and `Queue`s, as well as
concrete implementations for the aforementioned programs. The list of currently
supported programs is as follows:

## Programs

The following quantum chemistry programs are supported:

- [x] MOPAC
- [x] Molpro
- [ ] Gaussian

## Queuing systems

The following queuing systems are supported:

- [x] Slurm
- [x] PBS

