pub mod cli;
pub mod config;
pub mod db;
pub mod clients;
pub mod sync;
pub mod ipc;
pub mod error;

pub use error::{Error, Result};