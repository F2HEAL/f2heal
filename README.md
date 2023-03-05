# F2heal

## Compilation

The usage of this program requires a working Rust environment, refer to the [installation instructions](https://www.rust-lang.org/tools/install) for more details.

The crate [Flac-bound](https://crates.io/crates/flac-bound) is a dependency to generate the Flac output files. Please check the documentation of this crate for requirements specific to your platform.

## Usage

    $ cargo run -- -h


Generate 2 minute file with default settings

    $ cargo run -r -- -s120 -v
    
    
