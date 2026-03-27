/// The application entrypoint and top-level orchestration.
pub mod app;
/// The command-line argument surface.
pub mod cli;
/// The reduced-source extraction engine.
pub mod extract;
/// Filesystem traversal and input resolution helpers.
pub mod fs;
/// Source-language detection and parser configuration.
pub mod language;
/// The thin internal model that drives rendering.
pub mod model;
/// Text rendering for reduced-source file sections.
pub mod render;
/// Source text slicing and line-number helpers.
pub mod source;
