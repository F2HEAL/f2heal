use rand_chacha::ChaCha8Rng;
use rand::prelude::*;
use std::f64::consts::PI;
use flac_bound;
use std::fs::File;

use clap::{Parser};
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]

/// Create F2Heal FLAC audio output
struct Arguments {

    /// Channels or fingers per side (L/R), 
    #[arg(long, default_value_t = 4)]
    channels : i64,

    /// Output file sample rate in Hz
    #[arg(long, default_value_t = 44100)]
    samplerate : i64,

    /// Frequency of finger stimulation in Hz
    #[arg(long, default_value_t = 250)]
    stimfreq : i64,

    /// Duration of the finger stimulation in ms
    #[arg(long, default_value_t = 100)]
    stimperiod : i64,

    /// Duration of one cycle (stimulation of all fingers)
    #[arg(long, default_value_t = 666)]
    cycleperiod : i64,

    /// Select 'Simultaneous Stimulation'-mode (as opposed to default Blocked-mode) and set the random phase shift interval in ms. 
    /// Each cycle, all channels, but one, will recieve a random delay within this interval.
    #[arg(long)]
    phaseshift : Option<i64>,

    /// Select 'Simultaneous Stimulation'-mode (as opposed to default Blocked-mode) with fixed shift intervals (quarter of stim period randomized per channel)
    #[arg(short, long, default_value_t = false)]
    fixedphaseshift: bool,


    /// Number of repetitions before new random channel-pattern is calculated
    #[arg(short, long, default_value_t = 25)]
    repetitions: i64,

    /// Duration (in cycles) of one pauze-cycle
    #[arg(long, default_value_t = 5)]
    pauzecycleperiod : i64,

    /// The cycles (within the pauze-cycle) with no stimulation output produced. You can use this option more than once.
    #[arg(short, long)]
    pauzes : Vec<i64>,

    /// Duration in sec of output
    #[arg(short,long)]
    secondsoutput: i64,

    /// Random seed (default from timer)
    #[arg(long)]
    randomseed: Option<i64>,

    #[arg(long, default_value_t = false)]
    norandom: bool,


    /// Output verbosity. You can use this option more than once.
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbosity: u8,

}

impl Arguments {

    /// Verify the supplied arguments make sense for generating output
    fn verify_argvalues(&self) {

        // Do the stimulation frequency en period match, otherwise said, does the stimulation sine
        // end on period end
        let stimfreq_frame = 1000 / self.stimperiod;
        let smooth_stim_badend = (self.stimfreq % stimfreq_frame) != 0;

        if smooth_stim_badend {
            println!("\n{}",
                format!("WARNING: Stimulation period and frequency do not match!").red().bold());
        }

        if self.stimperiod * self.channels > self.cycleperiod {
            println!("\n{}",
                format!("WARNING: overlapping stimulation periods not supported!").red().bold());
        }

        // Are the selected pauzes within the pauze period
        for pauze in self.pauzes.iter() {
            if pauze >= &self.pauzecycleperiod {
                println!("\n{}",    
                    format!("WARNING: This pauze will have no effect: {}", pauze).red().bold(),
                );
            }
        }

        // Is the phaseshift small enough to allow stim signal to end before the next one starts
        if !self.phaseshift.is_none() {
            if (self.phaseshift.unwrap() + self.stimperiod) * self.channels > self.cycleperiod {
                println!("\n{}",    
                    format!("WARNING: Phase shift is too large: {}ms over limit", 
                    self.phaseshift.unwrap() + self.stimperiod -  self.cycleperiod / self.channels,   
                        ).red().bold(),
                );
            }
        }

        if !self.phaseshift.is_none() && self.fixedphaseshift {
            println!("\n{}",
                format!("ERROR: Conflicting command line options, choose either random of fixed phase shift mode.").red().bold());
            assert_eq!(self.phaseshift.is_none(), self.fixedphaseshift, "!!!ERROR: Conflict in command line");
            
        }

        // The 4 channels are hardcoded in several places, so force them on 4 for now...
        assert_eq!(self.channels,4,"!!!ERROR: Only 4 channels supported for now");

    }


    /// Display overview of configured parameters for this run1
    fn display_config(&self) {
        println!("Generating FLAC output for:");
        println!("   Channels [L/R]          : {}", self.channels);
        println!("   Sample Rate             : {}Hz", self.samplerate);
        println!("   Duration                : {}s", self.secondsoutput);
        println!("");
        println!("   Stimulation details:");
        println!("     Stimulation Frequency : {}Hz", self.stimfreq);
        println!("     Stimulation Period    : {}ms", self.stimperiod);
        println!("     Cycle Period          : {}ms", self.cycleperiod);
        println!("     Cycle repetitions     : {}", self.repetitions);
        println!("");
        if !self.phaseshift.is_none() {
            println!("     Phaseshifted, random interval : {}ms", self.phaseshift.unwrap());
        } else if self.fixedphaseshift {
            println!("     Phaseshifted, fixed interval");
        } else {
            println!("     Interleaved");
        }
        println!("");
        if self.pauzes.is_empty() {
            println!("   Without pauzes");
        } else {
            println!("   Pauze cycle period      : {}", self.pauzecycleperiod);
            println!("   Pauze on cycles         : {:?}", self.pauzes);
        }
        println!("");
        if self.randomseed.is_none() {
            println!("   Randomized seed");
        } else {
            println!("   Random seed             : {}", self.randomseed.unwrap());
        }
  
    }

    /// Set filename with all parameters included
    fn construct_fname(&self) -> String {
        let mut result: String = "output/Sine-".to_owned();

        if !self.phaseshift.is_none() {
            result.push_str(&self.phaseshift.unwrap().to_string());
            result.push_str("PhaseShifted--");
        } else if self.fixedphaseshift {
            result.push_str("FixPhaseShifted--"); 
        } else {
            result.push_str("Interleaved--");
        }

        result.push_str(&self.stimfreq.to_string());    result.push_str("SFREQ-");
        result.push_str(&self.stimperiod.to_string());  result.push_str("SPER-");
        result.push_str(&self.cycleperiod.to_string()); result.push_str("CPER-");
        result.push_str(&self.repetitions.to_string()); result.push_str("R--");

        if !self.pauzes.is_empty() {
            let mut first : bool = true;

            for pauze in self.pauzes.iter() {
                if first {
                    first = false;
                } else {
                    result.push_str("_");
                }

                result.push_str(&pauze.to_string()); 
            }
            result.push_str("P");
            result.push_str(&self.pauzecycleperiod.to_string());
            result.push_str("--");
        }

        if !self.randomseed.is_none() {
            result.push_str(&self.randomseed.unwrap().to_string());
            result.push_str("RSEED--");
        }

        result.push_str(&self.channels.to_string());      result.push_str("LR-");
        result.push_str(&self.samplerate.to_string());    result.push_str("Hz-");
        result.push_str(&self.secondsoutput.to_string()); result.push_str("s");

        result.push_str(".flac");

        result
    }

}



// TODO: this restricts channels to 4 (1)
type  AtomSeq = [i64; 4 as usize];

struct SeqGen {
    rng: ChaCha8Rng,
    sample : i64,
    cycle: i64,
    cyclestart: i64,
    repcycle: i64,
    channelorder : [ AtomSeq; 2],
}

impl SeqGen {

    /// Construct new SegGen from supplied arguments.
    fn new(args: &Arguments) -> SeqGen {

        let mut new_rng = ChaCha8Rng::from_entropy();
        if !args.randomseed.is_none() {
            new_rng = ChaCha8Rng::seed_from_u64(args.randomseed.unwrap() as u64);
        } 


        // TODO: this restricts channels to 4 (2)
        let seq = [ [0; 4], [0; 4] ];
        
        SeqGen { rng: new_rng, sample : 0, cycle: 0, cyclestart: 0, repcycle: 1, channelorder : seq }
    }

    /// Init SegGen1 state from supplied arguments
    fn init(&mut self, args: &Arguments) {
        if args.phaseshift.is_none() && !args.fixedphaseshift{
            self.gen_channelorder(args);
        } else {
            self.gen_phasedelay(args);
        }
    }

    /// Generates new random pattern for each hand (for interleaved mode - not phaseshifted)
    fn gen_channelorder(&mut self, args: &Arguments) {
        for h in 0..2 {
            let mut nums : AtomSeq = [0; 4];
            
            loop {
                let mut counter = 0;
                nums = nums.map(|_| { counter += 1; counter -1 });

                if not args.norandom {
                    nums.shuffle(&mut self.rng);
                }

                // this protects us from triggering the same finger twice in sequence
                if nums[0] != *self.channelorder[h].last().unwrap() {
                    break;
                }
            }

            self.channelorder[h] = nums;
        }

        if args.verbosity > 1 {
            println!(" * New Channel Order: {:?}-{:?}", self.channelorder[0], self.channelorder[1]);
        }

    }


    /// Generate new randomized phase delay for each channel (when phaseshift - not interleaved mode)
    fn gen_phasedelay(&mut self, args: &Arguments) {
        for h in 0..2 {
            let mut nums : AtomSeq = [0; 4];

            // we don't touch the first element, it will be the zero-delay one ico randomized delays
            for i in 1..4 {
                if args.fixedphaseshift {
                    nums[i] = i as i64 * 1_000 /  args.stimfreq / 4 * args.samplerate / 1_000;
                } else {
                    nums[i] = self.rng.gen_range(0..args.phaseshift.unwrap()) * args.samplerate / 1_000;
                }
            }

            nums.shuffle(&mut self.rng);

            self.channelorder[h] = nums;
        }

        if args.verbosity > 1 {
            println!(" * New Phase Shift: {:?}-{:?}", self.channelorder[0], self.channelorder[1]);
        }

    }

    // Set internal counter to next sample. Renew internal structures where necessary
    fn next_sample(&mut self, args: &Arguments) {
        self.sample += 1;

        if self.curr_cycle(args) < self.cycle  {
            // we went back to cycle 0:
            //  - generate new random pattern for both hands (unless phaseshift)

            if args.phaseshift.is_none() && !args.fixedphaseshift {
                if self.repcycle < args.repetitions {
                    self.repcycle += 1;
                } else {
                    self.repcycle = 1;
                    self.gen_channelorder(&args);
                }
            }  
        }

        if self.curr_cycle(args) != self.cycle {
            // cycle changed:
            //  - set cyclestart
            //  - generate delay per channel (for phaseshift)
            self.cyclestart = self.sample;

            if !args.phaseshift.is_none() || args.fixedphaseshift {
                if self.repcycle < args.repetitions {
                    self.repcycle += 1;
                } else {
                    self.repcycle = 1;
                    self.gen_phasedelay(&args);
                }
            }

            if args.verbosity > 2 {
                println!(" Cycle #{} at {}", self.curr_cycle(args), self.sample)
            }
        }

        self.cycle = self.curr_cycle(args);
    }

    /// Returns the current cycle (in range 0..args.channels)
    fn curr_cycle(&mut self, args: &Arguments) -> i64{
        ( self.sample * 1_000 * args.channels / args.samplerate / args.cycleperiod ) % args.channels
    }

    /// Returns whether channel is in pauze
    fn in_pauze(&self, args: &Arguments) -> bool {
        let curr_paucycle = ( self.sample * 1_000 / args.samplerate / args.cycleperiod ) % args.pauzecycleperiod;

        args.pauzes.contains(&curr_paucycle)
    }

    // Returns value of current sample for hand/channel combination
    fn sample(&mut self, args: &Arguments, hand: usize, channel: i64) -> f64 {
        if args.phaseshift.is_none() && !args.fixedphaseshift {
            self.sample_blocked(args, hand, channel)
        } else {
            self.sample_phaseshifted(args, hand, channel)
        }
    }

    /// Value of sample in phaseshifted mode
    fn sample_phaseshifted(&mut self, args: &Arguments, hand: usize, channel: i64) -> f64 {
        let cycle_active_from = self.cyclestart + self.channelorder[hand][channel as usize];
        let cycle_active_until = cycle_active_from + args.stimperiod * args.samplerate / 1_000;

        if self.sample > cycle_active_from && self.sample < cycle_active_until {
            let rel_sample = self.sample - cycle_active_from;
            
            let arg = rel_sample * args.stimfreq * 2;
            (arg as f64 * PI / args.samplerate as f64).sin()
        } else {
            0.0
        }
    }

    /// Value of sample in blocked mode
    fn sample_blocked(&mut self, args: &Arguments, hand: usize, channel: i64) -> f64 {
        let active_channel = self.channelorder[hand][self.cycle as usize];

        if channel != active_channel {
            return 0.0;
        }

        let cycle_active_time = args.stimperiod * args.samplerate / 1000;

        let rel_sample = self.sample - self.cyclestart; 

        if rel_sample > cycle_active_time {
            return 0.0;
        }

        let arg = rel_sample * args.stimfreq * 2;
        (arg as f64 * PI / args.samplerate as f64).sin()
    } 
        
}


fn main() {

    let args = Arguments::parse();
 
    if args.verbosity > 0 {
        args.display_config();
    }
    args.verify_argvalues();

    let fname = args.construct_fname();

    println!("Writing output to: {}", fname);

    let samples_to_go = args.secondsoutput * args.samplerate;
  
    let mut flac_outfile = File::create(fname).unwrap();
    let mut flac_outwrap = flac_bound::WriteWrapper(&mut flac_outfile);
    let mut flac_encoder = flac_bound::FlacEncoder::new().unwrap()
        .channels((2*args.channels).try_into().unwrap())
        .bits_per_sample(16)
        .sample_rate(args.samplerate as u32)
        .total_samples_estimate(samples_to_go as u64)
        .compression_level(8)
        .init_write(&mut flac_outwrap)
        .unwrap();

    let mut seq1 = SeqGen::new(&args);
    seq1.init(&args);

    for _ in 0..samples_to_go {
        let mut next_sample : [i32; 2*4 as usize] = [0; 2*4 as usize];

        if !seq1.in_pauze(&args) {
            for hand in 0..2 {  
                for channel in 0..4 {    
                    let sample = seq1.sample(&args, hand as usize, channel);
                    let amplitude = i16::MAX as f64;
                        
                    next_sample[(channel + hand * 4) as usize] = (sample*amplitude) as i32;
                }
            }
        }

        flac_encoder.process_interleaved(&next_sample,1).unwrap();
        
        seq1.next_sample(&args); 
    }

    
}
