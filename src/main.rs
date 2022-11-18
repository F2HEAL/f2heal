//use rand;
use rand_chacha::ChaCha8Rng;
use rand::prelude::*;
use std::f64::consts::PI;
use std::i16;
use hound;


// Golden values:
// const SAMPLERATE   : u32 = 44100;
// const CHANNELS     : u16 = 4;      // = #fingers
// const RANDPATTERNS : u8  = 6;      // number of patterns before cycle
// const STIMFREQ     : u32 = 250;    // Stimulation frequency in Hz
// const STIMPERIOD   : u32 = 100;    // Stimulation period of single channel in ms
// const CYCLEPERIOD  : u32 = 666;    // Stimulation period in ms

const SAMPLERATE   : u32 = 44100;
const CHANNELS     : u16 = 4;      // = #fingers
const RANDPATTERNS : u8  = 6;      // number of patterns before cycle
const STIMFREQ     : u32 = 250;    // Stimulation frequency in Hz
const STIMPERIOD   : u32 = 100;    // Stimulation period of single channel in ms
const CYCLEPERIOD  : u32 = 666;   // Stimulation period in ms

const SECONDSOUTPUT: u32 = 900;     // Duration of output wav
const RANDOMSEED   : u64 = 3;      // Seed to contract random pattern generation

type  AtomSeq = [u8; CHANNELS as usize];

struct SeqGen {
    currsample : u32,
    fseq : [ Vec<AtomSeq>; 2],
}

impl SeqGen {
    fn new() -> SeqGen {
        //let mut rng = rand::thread_rng();
        let mut rng = ChaCha8Rng::seed_from_u64(RANDOMSEED);

        let mut seq : [Vec<AtomSeq>;2 ] = [ Vec::new() ,  Vec::new() ];

        for h in 0..1 {
            for _ in 0..RANDPATTERNS {
                let mut nums : AtomSeq = [0; CHANNELS as usize];
                for i in 0..CHANNELS {  //todo: improve this
                    nums[i as usize] = i as u8;
                }
                nums.shuffle(&mut rng);

                seq[h].push(nums);
            }
        }

        SeqGen { currsample : 0, fseq : seq }
    }

    fn next_sample(&mut self) {
        self.currsample += 1;
    }

    fn sample(&self, hand: usize, channel: u8) -> f64 {
        //let currtime = self.currsample as f64 / SAMPLERATE as f64; //where are we in seconds time?


        // which is the current cycle we're running in? (don't wrap yet at RANDPATTERNS as we 
        // need the relative sample within the cycle, see below)
        let currcycle = (self.currsample as f64 / SAMPLERATE as f64 * 1000.0 / CYCLEPERIOD as f64) as u32; 
        let currcycle_order = currcycle % RANDPATTERNS as u32; // wrap at RANDPATTERNS for knowing which cycle-pattern to sellect

        // how many samples far in the current cycle are we?
        let cycle_sample = self.currsample - currcycle * ( SAMPLERATE * CYCLEPERIOD / 1000);

        // which channel# should be playing now?
        let currchan_order = (CHANNELS as f64 * cycle_sample as f64 / ( SAMPLERATE as f64* CYCLEPERIOD  as f64/ 1000.0)) as u32;
        let currchan_order = if currchan_order >= CHANNELS.into() { (CHANNELS - 1) as u32 } else { currchan_order as u32 };  // squash rounding error


        
        // randomize channel through selected order
        let currchan = self.fseq[hand][currcycle_order as usize][currchan_order as usize] as u32;

        if u32::from(channel) != currchan { //another channel is active currently
            return 0.0;
        }

        let chan_sample = cycle_sample - currchan_order * ( SAMPLERATE * CYCLEPERIOD / ( CHANNELS as u32 * 1000)); // relative sample number whitin current channel

        let chansim_active = chan_sample < (STIMPERIOD * SAMPLERATE / 1000);

        //println!("sample #{} is in cycle {} at chan {}, active {} chansample #{} with value {} at {} sec ", self.currsample, currcycle, currchan, chansim_active, chan_sample, (chan_sample as f64 * STIMFREQ as f64 * 2.0 * PI).sin(), currtime,);

        if !chansim_active { //we're in the active channel, but the STIMPERIOD has passed already
            return 0.0;
        }

        (chan_sample as f64 * STIMFREQ as f64 * 2.0 * PI / SAMPLERATE as f64).sin()
    }
}

fn main() {
    let mut seq1 = SeqGen::new();

    println!("Generating {} random patterns:", RANDPATTERNS);
    for hand in 0..1 {
        for xs in seq1.fseq[hand].iter() {
            println!("Array {} {:?}", hand, xs);
        }
    }

    //set filename with all parameters included
    let fname = "output/sine-2hands-".to_string() + &CHANNELS.to_string() + &"chan-".to_string() 
        + &RANDPATTERNS.to_string() + &"patterns-".to_string() + &STIMFREQ.to_string() + &"SFREQ-".to_string() + &STIMPERIOD.to_string() 
        + &"SPER-".to_string() + &CYCLEPERIOD.to_string() + &"CPER-WAV".to_string() + &SAMPLERATE.to_string() + &"Hz-16bit-signed.wav".to_string();    

    println!("\nWriting {}sec output to: {}", SECONDSOUTPUT, fname);

    // setup wav stream
    let wavspec = hound::WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLERATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(fname, wavspec).unwrap();
    
    

    let samples_to_go = SECONDSOUTPUT * SAMPLERATE;

    for _ in 0..samples_to_go {
        for channel in 0..CHANNELS {
            for hand in 0..1 {
                let sample = seq1.sample(hand, channel as u8);
                let amplitude = i16::MAX as f64;
                
                //println!("Sample #{} a chan {} has value: {} with duration {}", seq1.currsample, channel, sample*amplitude, writer.duration());
                writer.write_sample((sample*amplitude) as i16).unwrap();
            }
        }
        seq1.next_sample(); 
    }
}
