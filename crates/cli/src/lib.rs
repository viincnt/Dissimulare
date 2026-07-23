//! Library half of the Dissimulare CLI: the `setup`/`start`/`run`/`status`/
//! `uninstall` logic, reused as-is by both the `dissimulare` binary
//! (`src/main.rs`) and `dissimulare-tui`. Neither caller reimplements any of
//! this — they only call it.

pub mod cli;
pub mod commands;
pub mod config;
pub mod seed;
