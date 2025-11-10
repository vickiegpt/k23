// Copyright 2025 Jonas Kruckenberg
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::env;

use vergen_gitcl::{BuildBuilder, CargoBuilder, Emitter, GitclBuilder, RustcBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For x86_64, compile the assembly entry point
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("x86_64") {
        println!("cargo:rerun-if-changed=src/arch/x86_64/entry.s");
        cc::Build::new()
            .file("src/arch/x86_64/entry.s")
            .compile("entry");
    }

    let build = BuildBuilder::default().build_timestamp(true).build()?;
    let cargo = CargoBuilder::default()
        .target_triple(true)
        .opt_level(true)
        .build()?;
    let rustc = RustcBuilder::default().semver(true).channel(true).build()?;
    let git = GitclBuilder::default()
        .sha(true)
        .commit_timestamp(true)
        .branch(true)
        .build()?;

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&cargo)?
        .add_instructions(&git)?
        .add_instructions(&rustc)?
        .emit()?;

    Ok(())
}
