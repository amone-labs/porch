//! Shared core for porch: the event format written by `porch-hook`,
//! where files live, and hook installation into agent config files.

pub mod activity;
pub mod agents;
pub mod board;
pub mod cli_link;
pub mod digest;
pub mod event;
pub mod export;
pub mod focus;
pub mod goals;
pub mod health;
pub mod insight;
pub mod insight_eval;
pub mod install;
pub mod lang;
pub mod limits;
pub mod mirror;
pub mod open;
pub mod paths;
pub mod schedule;
pub mod settings;
pub mod state;
pub mod statusline;
pub mod suggest;
pub mod summary;
pub mod time;
pub mod usage;
pub mod writer;
