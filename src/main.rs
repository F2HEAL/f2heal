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

type  AtomSeq = [i64; CHANNELS as usize];

struct SeqGen {
    rng: ChaCha8Rng,
    sample : i64,
    cycle: i64,
    cyclestart: i64,
    channelorder : [ AtomSeq; 2],
}

impl SeqGen {
    fn new() -> SeqGen {
        //let mut rng = rand::thread_rng();
        let new_rng = ChaCha8Rng::seed_from_u64(RANDOMSEED);

        let seq = [ [0; 4], [0; 4] ];
        
        SeqGen { rng: new_rng, sample : 0, cycle: 0, cyclestart: 0, channelorder : seq }
    }

    // Generates new random pattern for each hand
    fn gen_channelorder(&mut self) {
        for h in 0..2 {
            let mut nums : AtomSeq = [i64::MAX; CHANNELS as usize];
            
            loop {
                for i in 0..CHANNELS{
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

        //println!(" * New Pattern: {:?}-{:?}", self.channelorder[0], self.channelorder[1]);

    }

    fn next_sample(&mut self) {
        self.sample += 1;

        if self.curr_cycle() < self.cycle  {
            // we went back to cycle 0:
            //  - generate new random pattern for both hands

            self.gen_channelorder();
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

    //set filename with all parameters included
    let fname = "output/sine-2hands-pauzed-".to_string() + &CHANNELS.to_string() + &"chan-".to_string() 
        + &STIMFREQ.to_string() + &"SFREQ-".to_string() + &STIMPERIOD.to_string() 
        + &"SPER-".to_string() + &CYCLEPERIOD.to_string() + &"CPER-WAV".to_string() + &SAMPLERATE.to_string() + &"Hz-16bit-signed.wav".to_string();    

    println!("\nWriting {}sec output to: {}", SECONDSOUTPUT, fname);

    // setup wav stream
    let wavspec = hound::WavSpec {
        channels: 2*CHANNELS as u16,
        sample_rate: SAMPLERATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(fname, wavspec).unwrap();
    

    let samples_to_go : i64 = SECONDSOUTPUT * SAMPLERATE;


    let mut seq1 = SeqGen::new();
    seq1.gen_channelorder();

    for _ in 0..samples_to_go {
        for hand in 0..2 {  
            for channel in 0..CHANNELS {
                if seq1.in_pauze() {
                    writer.write_sample(0).unwrap();    
                } else {
                    let sample = seq1.sample(hand, channel);
                    let amplitude = i16::MAX as f64;
                
                    //println!("Sample #{} a chan {} has value: {} with duration {}", seq1.sample, channel, sample*amplitude, writer.duration());
                    writer.write_sample((sample*amplitude) as i16).unwrap();
                }
            }
        }
        seq1.next_sample(); 
    }
}
