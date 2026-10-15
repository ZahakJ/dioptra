// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Records the target triple for the OQ report (`oq.rs`).
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=DIOPTRA_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
