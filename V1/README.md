# V1

This is the first implementation of the f2heal FLAC generator.

Output will be a FLAC file with 8 channels, grouped as 2 x 4 channels. The 4 channels in each group are activated independently, each channel is intended to drive an actuator for a finger on the left or right hand.

This version implements 3 modes, to be selected from the command line:
* blocked or interleaved mode
* phase shifted
* fix phase shifted mode

Please refer to the Doc folder for more details

## Usage

Go to the **V1** directory and create an **output/** directory.

To see the command line options

    $ cargo run -- -h


Generate 2 minute file with default settings

    $ cargo run -r -- -s120 -v

Please read [this page](https://crates.io/crates/flac-bound) if you get the following compilation error:

    = note: /usr/bin/ld: cannot find -lflac: No such file or directory


    
