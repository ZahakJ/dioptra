//! Transcendental functions through `libm` (a pure-Rust port of musl) instead of the
//! platform C library, so every operating system and the browser build compute the same
//! bits and the SVG output is byte-identical everywhere (DESIGN §1: "a graph rendered on
//! Linux is byte-identical on a Mac").

/// Deterministic versions of the `f64` methods the engine uses.
pub trait DetMath {
    /// eˣ.
    fn exp_d(self) -> f64;
    /// Natural log.
    fn ln_d(self) -> f64;
    /// Base-10 log.
    fn log10_d(self) -> f64;
    /// Base-2 log.
    fn log2_d(self) -> f64;
    /// Log to a base.
    fn log_d(self, base: f64) -> f64;
    /// xʸ.
    fn powf_d(self, y: f64) -> f64;
    /// Sine.
    fn sin_d(self) -> f64;
    /// Cosine.
    fn cos_d(self) -> f64;
    /// (sine, cosine).
    fn sin_cos_d(self) -> (f64, f64);
}

impl DetMath for f64 {
    fn exp_d(self) -> f64 {
        libm::exp(self)
    }
    fn ln_d(self) -> f64 {
        libm::log(self)
    }
    fn log10_d(self) -> f64 {
        libm::log10(self)
    }
    fn log2_d(self) -> f64 {
        libm::log2(self)
    }
    fn log_d(self, base: f64) -> f64 {
        libm::log(self) / libm::log(base)
    }
    fn powf_d(self, y: f64) -> f64 {
        libm::pow(self, y)
    }
    fn sin_d(self) -> f64 {
        libm::sin(self)
    }
    fn cos_d(self) -> f64 {
        libm::cos(self)
    }
    fn sin_cos_d(self) -> (f64, f64) {
        (libm::sin(self), libm::cos(self))
    }
}
