//use rand;
use rand_chacha::ChaCha8Rng;
use rand::prelude::*;
use std::{f64::consts::PI, io::BufWriter};
use std::i16;
//use flac_bound::{WriteWrapper, FlacEncoder};
use flac_bound;
use std::fs::File;
use hound::{self, WavWriter};

use clap::{Parser, Arg};
use colored::Colorize;


// Golden values:
// const SAMPLERATE   : u32 = 44100;
// const CHANNELS     : u16 = 4;      // = #fingers
// const RANDPATTERNS : u8  = 6;      // number of patterns before cycle
// const STIMFREQ     : u32 = 250;    // Stimulation frequency in Hz
// const STIMPERIOD   : u32 = 100;    // Stimulation period of single channel in ms
// const CYCLEPERIOD  : u32 = 666;    // Stimulation period in ms

const SAMPLERATE   : i64 = 44100;
const CHANNELS     : i64 = 4;      // = #fingers
const STIMFREQ     : i64 = 250;    // Stimulation frequency in Hz
const STIMPERIOD   : i64 = 100;    // Stimulation period of single channel in ms
const CYCLEPERIOD  : i64 = 666;   // Stimulation period in ms

const PAUCYCLE     : i64 = 5;
const PAUZES       : [i64; 2] = [3, 4]; 
//const PAUZES       : [i64; 0] = [ ]; 

const SECONDSOUTPUT: i64 = 7200;   // Duration of output wav
const RANDOMSEED   : u64 = 4;      // Seed to contract random pattern generation

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

    /// Duration (in cycles) of one pauze-cycle
    #[arg(long, default_value_t = 5)]
    pauzecycleperiod : i64,

    /// The cycles (within the pauze-cycle) with no stimulation output produced
    #[arg(short, long)]
    pauzes : Vec<i64>,

    /// Duration in sec of output
    #[arg(short,long, default_value_t = 90)]
    secondsoutput: i64,

    /// Random seed (default from timer)
    #[arg(short, long)]
    randomseed: Option<i64>,

    /// Create WAV outputfile in stead of default FLAC
    #[arg(short,long, default_value_t = false)]
    wavoutput: bool,

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

        assert_eq!(self.channels,4,"!!!ERROR: Only 4 channels supported for now");

    }


    fn display_config(&self) {
        println!("Generating output for:");
        println!("   Channels [L/R]          : {}", self.channels);
        println!("   Sample Rate             : {}Hz", self.samplerate);
        println!("   Duration                : {}s", self.secondsoutput);
        println!("   Format                  : {}", if self.wavoutput { "WAV" } else { "FLAC"});
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

        self.verify_argvalues();
    }

    fn construct_fname(&self) -> String {
        let mut result: String = "output/sine-".to_owned();

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

        if self.wavoutput {
            result.push_str(".wav");
        } else {
            result.push_str(".flac");
        }

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

    // Generates new random pattern for each hand
    fn gen_channelorder(&mut self, args: &Arguments) {
        for h in 0..2 {
            let mut nums : AtomSeq = [i64::MAX; CHANNELS as usize];
            
            loop {
                for i in 0..args.channels{
                    nums[i as usize] =  i;
                }

                nums.shuffle(&mut self.rng);

                // this protects us from triggering the same finger twice in sequence
                if nums[0] != *self.channelorder[h].last().unwrap() {
                    break;
                }
            }

            self.channelorder[h] = nums;
        }

        if args.verbosity > 1 {
            println!(" * New Pattern: {:?}-{:?}", self.channelorder[0], self.channelorder[1]);
        }

    }

    fn next_sample(&mut self, args: &Arguments) {
        self.sample += 1;

        if self.curr_cycle() < self.cycle  {
            // we went back to cycle 0:
            //  - generate new random pattern for both hands

            self.gen_channelorder(&args);
        }

        if self.curr_cycle() != self.cycle {
            // cycle changed:
            //  - set cyclestart
            self.cyclestart = self.sample;
        }

        self.cycle = self.curr_cycle();
    }

    fn curr_cycle(&mut self) -> i64{
        ( self.sample * 1_000 * CHANNELS / SAMPLERATE  / CYCLEPERIOD ) % CHANNELS
    }

    fn in_pauze(&self) -> bool {
        let curr_paucycle = ( self.sample * 1_000 / SAMPLERATE  / CYCLEPERIOD ) % PAUCYCLE;

        PAUZES.contains(&curr_paucycle)

    }

    fn sample(&mut self, hand: usize, channel: i64) -> f64 {
        let active_channel = self.channelorder[hand][self.cycle as usize];

        if channel != active_channel {
            return 0.0;
        }

        let cycle_active_time = STIMPERIOD * SAMPLERATE / 1000;

        let rel_sample = self.sample - self.cyclestart; 

        if rel_sample > cycle_active_time {
            return 0.0;
        }

        let arg = rel_sample * STIMFREQ * 2;
        (arg as f64 * PI / SAMPLERATE as f64).sin()
    } 
        
}


fn main() {

    let args = Arguments::parse();
    println!("{:?}", args);

    args.display_config();

    //set filename with all parameters included
    let fname = args.construct_fname();

    println!("\nWriting output to: {}", fname);

    //let samples_to_go : i64 = SECONDSOUTPUT * SAMPLERATE;
    let samples_to_go = args.secondsoutput * args.samplerate;
    

    //let mut wav_encoder  = hound::WavWriter::<std::io::BufWriter<File>>::new();
    //let mut wav_encoder  = hound::WavWriter::create("/dev/null", hound::WavSpec { channels: 1, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int}).unwrap();
    let mut wav_encoder: Option<WavWriter<BufWriter<File>>> = None;
    //let mut flac_outfile = File::create("/dev/null").unwrap();
    //let mut flac_outwrap = flac_bound::WriteWrapper(&mut flac_outfile);
    //let mut flac_encoder = flac_bound::FlacEncoder::new().unwrap().init_write(&mut flac_outwrap).unwrap(); 
    let mut flac_outfile;
    let mut flac_outwrap;
    let mut flac_encoder;

    let cwavoutput = args.wavoutput;
    if cwavoutput {
        // setup wav stream
        let wavspec = hound::WavSpec {
            channels: 2*args.channels as u16,
            sample_rate: args.samplerate as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        wav_encoder = Some(hound::WavWriter::create(fname, wavspec).unwrap());
    } else {
        flac_outfile = File::create(fname).unwrap();
        flac_outwrap = flac_bound::WriteWrapper(&mut flac_outfile);
        flac_encoder = flac_bound::FlacEncoder::new().unwrap()
        .channels((2*args.channels).try_into().unwrap())
        .bits_per_sample(16)
        .total_samples_estimate(samples_to_go as u64)
        .compression_level(8)
        .init_write(&mut flac_outwrap)
        .unwrap();
    }


    
    //let mut enc 
    //eprintln!("{:?}", enc);

    let mut seq1 = SeqGen::new(&args);
    seq1.gen_channelorder(&args);

    for _ in 0..samples_to_go {
        let mut next_sample : [i32; 2*CHANNELS as usize] = [0; 2*CHANNELS as usize];

        for hand in 0..2 {  
            for channel in 0..CHANNELS {
                if !seq1.in_pauze() {
                    
                    let sample = seq1.sample(hand as usize, channel);
                    let amplitude = i16::MAX as f64;
                
                    //println!("Sample #{} a chan {} has value: {} with duration {}", seq1.sample, channel, sample*amplitude, writer.duration());
                        
                    next_sample[(channel + hand * CHANNELS) as usize] = (sample*amplitude) as i32;
                }
            }
        }

        if cwavoutput { 
            for chan_sample in next_sample.iter() {
                wav_encoder.unwrap().write_sample(*chan_sample as i16).unwrap();
            }

        } else {
            flac_encoder.process_interleaved(&next_sample,1).unwrap();
        }
        seq1.next_sample(&args); 
    }

    
}
