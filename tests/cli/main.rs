//! Black-box CLI contract suite.
//!
//! One integration-test binary, split by subject. Add a new test to the module
//! that owns the behaviour it asserts; see `AGENTS.md` for the map.

mod common;

mod add;
mod archive;
mod auto_capture;
mod contract;
mod digest;
mod docs;
mod doctor;
mod dogear;
mod export;
mod hook;
mod legacy;
mod list;
mod redaction;
mod resolve;
mod retrospect;
mod stderr_file;
mod store;
mod sweep;
mod triage;
mod verify;
