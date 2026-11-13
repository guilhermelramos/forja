//! Motor VT da Forja: grade, parser e imagens embutidas.
//!
//! A superfície é independente de renderização; a view lê as células e resolve
//! cores pela paleta escolhida nas configurações.

pub mod grid;
pub mod image;
pub mod parser;
mod sgr;
pub mod surface;

pub use grid::{attrs, Glyph, Row};
pub use surface::{Signal, Surface};
