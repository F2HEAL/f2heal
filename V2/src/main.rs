use rand_chacha::ChaCha8Rng;
use rand::prelude::*;

use flac_bound;
use std::fs::File;

use clap::{Parser};
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]

/// Create F2Heal FLAC audio output
struct Arguments {

    /// Channels or fingers per side (L/R), 
    #[arg(long, default_value_t = 8)]
    channels : u32,

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
    #[arg(long, default_value_t = 888)]
    cycleperiod : i64,

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

   /// Disable randomization of channels in blocked mode, and thus plays channels in order 1->2->3->4
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

        if self.stimperiod * self.channels as i64 > self.cycleperiod {
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
    }

    fn display_config(&self) {
        println!("Generating Blocked/Interleaved FLAC output for:");
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

    /// Set filename with all parameters included
    fn construct_fname(&self) -> String {
        let mut result: String = "output/Sine-Blocked".to_owned();

        result.push_str(&self.stimfreq.to_string());    result.push_str("SFREQ-");
        result.push_str(&self.stimperiod.to_string());  result.push_str("SPER-");
        result.push_str(&self.cycleperiod.to_string()); result.push_str("CPER-");

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

#[derive(Debug)]
struct SampleGenerator {
    rng: ChaCha8Rng,
    sample: i64,
    cycle: i64,
    cyclestart: i64,
    channelorder : Vec<u32>,
}

impl SampleGenerator {

    /// Constructor from cmdline args
    fn new(args: &Arguments) -> SampleGenerator {
        let mut rng = ChaCha8Rng::from_entropy();
        if !args.randomseed.is_none() {
            rng = ChaCha8Rng::seed_from_u64(args.randomseed.unwrap() as u64);
        }
        
        let channelorder = (0..args.channels).collect();

        SampleGenerator {
            rng, 
            sample: 0, 
            cycle: 0, 
            cyclestart: 0,
            channelorder
        }
    }

    /// Generates new random pattern for each hand 
    fn gen_channelorder(&mut self, args: &Arguments) {
        let mut channelorder : Vec<u32> = (0..args.channels).collect();
        
        if !args.norandom {
            // avoid triggering same channel twice 
            loop {
                channelorder.shuffle(&mut self.rng);

                if channelorder[0] != *self.channelorder.last().unwrap() {
                    break;
                }
            }
        }

        self.channelorder = channelorder;
    
        if args.verbosity > 1 {
            println!(" * New Channel Order: {:?}", self.channelorder[0]);
        }

    }


    /// Returns current sample for channel
    fn sample(&mut self, args: &Arguments, channel: u32) -> f64 {
        0.0
    }

    /// Returns whether channel is currently pauzed
    fn in_pauze(&self, args: &Arguments) -> bool {
        let curr_paucycle = ( self.sample * 1_000 / args.samplerate / args.cycleperiod ) % args.pauzecycleperiod;

        args.pauzes.contains(&curr_paucycle)
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
        .channels(args.channels)
        .bits_per_sample(16)
        .sample_rate(args.samplerate as u32)
        .total_samples_estimate(samples_to_go as u64)
        .compression_level(8)
        .init_write(&mut flac_outwrap)
        .unwrap();


    let mut sg = SampleGenerator::new(&args);

    println!("SampleGenerator: {:?}", sg);

    for _ in 0..samples_to_go {
        let mut next_sample = Vec::default();

        for channel in 0..args.channels {
            if sg.in_pauze(&args) {
                next_sample.push(0);
            } else {
                let sample = sg.sample(&args, channel);
                let amplitude = i16::MAX as f64;

                next_sample.push((sample * amplitude) as i32);
        
            }
        }
        flac_encoder.process_interleaved(&next_sample,1).unwrap();
    }

}



