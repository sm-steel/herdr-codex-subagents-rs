//! Domain contracts for a read-only Codex agent monitor.
//!
//! Observation is separate from terminal rendering and Herdr process control.
#![forbid(unsafe_code)]

pub mod agent;
pub mod record_buffer;

pub mod text;

pub mod rollout;

pub mod attachment;
pub mod herdr;
pub mod panel;
pub mod setup;
