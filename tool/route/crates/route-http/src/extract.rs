//! Axum extractors for Route state.
//!
//! Currently minimal — handlers use `State<SharedState>` directly
//! and call `state.open_repo()` to access the repository.

pub use crate::app_state::SharedState;
