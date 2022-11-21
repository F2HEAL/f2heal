//use rand;
use rand_chacha::ChaCha8Rng;
use rand::prelude::*;
use std::f64::consts::PI;
use std::i16;
//use flac_bound::{WriteWrapper, FlacEncoder};
use flac_bound;
use std::fs::File;

use clap::{Parser};
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// Create F2Heal audio output
struct Arguments {

    /// Channels or fingers per side (L/R), 
    #[arg(short, long, default_value_t = 4)]
    channels : i64,

    /// Sample rate in Hz
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

    /// Use simultaneous stimulation with following phase shift interval in ms
    #[arg(long)]
    phaseshift : Option<i64>,

    /// Duration (in cycles) of one pauze-cycle
    #[arg(long, default_value_t = 5)]
    pauzecycleperiod : i64,

    /// The cycles (within the pauze-cycle) with no stimulation output produced
    #[arg(short, long)]
    pauzes : Vec<i64>,

    /// Duration in sec of output
    #[arg(short,long)]
    secondsoutput: i64,

    /// Random seed (default from timer)
    #[arg(short, long)]
    randomseed: Option<i64>,

    /// Output verbosity (multiple allowed)
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbosity: u8,

}

impl Arguments {
    fn verify_argvalues(&self) {
        // Do the stimulation frequency en period match, otherwise said, does the stimulation sine
        // end on period end

        let stimfreq_frame = 1000 / self.stimperiod;
        let smooth_stim_badend = (self.stimfreq % stimfreq_frame) != 0;

        if smooth_stim_badend {
            println!("\n{}",
                format!("WARNING: Stimulation period and frequency do not match!").red().bold());
        }

        for pauze in self.pauzes.iter() {
            if pauze >= &self.pauzecycleperiod {
                println!("\n{}",    
                    format!("WARNING: This pauze will have no effect: {}", pauze).red().bold(),
                );
            }
        }

        if !self.phaseshift.is_none() {
            if (self.phaseshift.unwrap() + self.stimperiod) * self.channels > self.cycleperiod {
                println!("\n{}",    
                    format!("WARNING: Phase shift is too large: {}ms over limit", 
                    self.phaseshift.unwrap() + self.stimperiod -  self.cycleperiod / self.channels,   
                        ).red().bold(),
                );
            }
        }

        assert_eq!(self.channels,4,"!!!ERROR: Only 4 channels supported for now");

    }


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

    fn construct_fname(&self) -> String {
        let mut result: String = "output/Sine-".to_owned();

        if self.phaseshift.is_none() {
            result.push_str("Blocked-");
        } else {
            result.push_str(&self.phaseshift.unwrap().to_string());
            result.push_str("PhaseShifted-");
        }

        result.push_str(&self.stimfreq.to_string());    result.push_str("SFREQ-");
        result.push_str(&self.stimperiod.to_string());  result.push_str("SPER-");
        result.push_str(&self.cycleperiod.to_string()); result.push_str("CPER--");

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
    channelorder : [ AtomSeq; 2],
}

impl SeqGen {
    fn new(args: &Arguments) -> SeqGen {

        let mut new_rng = ChaCha8Rng::from_entropy();
        if !args.randomseed.is_none() {
            new_rng = ChaCha8Rng::seed_from_u64(args.randomseed.unwrap() as u64);
        } 


        // TODO: this restricts channels to 4 (2)
        let seq = [ [0; 4], [0; 4] ];
        
        SeqGen { rng: new_rng, sample : 0, cycle: 0, cyclestart: 0, channelorder : seq }
    }

    fn init(&mut self, args: &Arguments) {
        if args.phaseshift.is_none() {
            self.gen_channelorder(args);
        } else {
            self.gen_phasedelay(args);
        }
    }

    /// Generates new random pattern for each hand (unless phaseshift)
    fn gen_channelorder(&mut self, args: &Arguments) {
        for h in 0..2 {
            let mut nums : AtomSeq = [0; 4];
            
            loop {
                let mut counter = 0;
                nums = nums.map(|_| { counter += 1; counter -1 });

                nums.shuffle(&mut self.rng);

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


    /// Generate new randomized phase delay for each channel (for phaseshift)
    fn gen_phasedelay(&mut self, args: &Arguments) {
        for h in 0..2 {
            let mut nums : AtomSeq = [0; 4];

            // we don't touch the last element
            for i in 0..3 {
                nums[i] = self.rng.gen_range(0..args.phaseshift.unwrap())
            }

            nums.shuffle(&mut self.rng);

            self.channelorder[h] = nums;
        }

        if args.verbosity > 1 {
            println!(" * New Phase Shift: {:?}-{:?}", self.channelorder[0], self.channelorder[1]);
        }

    }


    fn next_sample(&mut self, args: &Arguments) {
        self.sample += 1;

        if self.curr_cycle(args) < self.cycle  {
            // we went back to cycle 0:
            //  - generate new random pattern for both hands (unless phaseshift)

            if args.phaseshift.is_none() {
                self.gen_channelorder(&args);
            }  
        }

        if self.curr_cycle(args) != self.cycle {
            // cycle changed:
            //  - set cyclestart
            //  - generate delay per channel (for phaseshift)
            self.cyclestart = self.sample;

            if !args.phaseshift.is_none() {
                self.gen_phasedelay(&args);
            }

            if args.verbosity > 2 {
                println!(" Cycle #{} at {}", self.curr_cycle(args), self.sample)
            }
        }

        self.cycle = self.curr_cycle(args);
    }

    fn curr_cycle(&mut self, args: &Arguments) -> i64{
        ( self.sample * 1_000 * args.channels / args.samplerate / args.cycleperiod ) % args.channels
    }

    fn in_pauze(&self, args: &Arguments) -> bool {
        let curr_paucycle = ( self.sample * 1_000 / args.samplerate / args.cycleperiod ) % args.pauzecycleperiod;

        args.pauzes.contains(&curr_paucycle)
    }

    fn sample(&mut self, args: &Arguments, hand: usize, channel: i64) -> f64 {
        if args.phaseshift.is_none() {
            self.sample_blocked(args, hand, channel)
        } else {
            self.sample_phaseshifted(args, hand, channel)
        }
    }

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

    //set filename with all parameters included
    let fname = args.construct_fname();

    println!("Writing output to: {}", fname);

    //let samples_to_go : i64 = SECONDSOUTPUT * SAMPLERATE;
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

        for hand in 0..2 {  
            for channel in 0..4 {
                if !seq1.in_pauze(&args) {
                    
                    let sample = seq1.sample(&args, hand as usize, channel);
                    let amplitude = i16::MAX as f64;
                
                    //println!("Sample #{} a chan {} has value: {} with duration {}", seq1.sample, channel, sample*amplitude, writer.duration());
                        
                    next_sample[(channel + hand * 4) as usize] = (sample*amplitude) as i32;
                }
            }
        }

        flac_encoder.process_interleaved(&next_sample,1).unwrap();
        
        seq1.next_sample(&args); 
    }

    
}
