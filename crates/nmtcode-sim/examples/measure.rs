//! Measures the reader on synthetic camera frames.
//!
//! ```sh
//! cargo run -p nmtcode-sim --release --example measure
//! cargo run -p nmtcode-sim --release --example measure -- --trials-a 1000 --trials-sweep 200
//! cargo run -p nmtcode-sim --release --example measure -- --only a
//! cargo run -p nmtcode-sim --release --example measure -- --only a --list 1
//! ```
//!
//! `--list 1` prints every trial of condition A that did not decode, with what the reader
//! found: for each finder (TL, TR, BL, BR) whether the finder search on the frame as captured
//! found it (`o`), found it as another kind (`m`) or missed it (`-`), and the errors of the
//! symbols it rejected.
//!
//! Condition A: 1920 × 1080, k = 3, σ = 0.5 module, 30 dB, JPEG quality 80, tilt up to 30°,
//! any rotation, symbol sides 24 to 100 (the other side within a factor of 2), levels 0 and 1,
//! a bootstrap QR Code beside a quarter of the symbols, cluttered background, mild uneven
//! light. The sweep changes k, σ and the tilt (fixed at 0°, 30° or 45°) of condition A; at
//! k = 2 and 2.5 it also counts, for each finder, how often it was not found or was classified
//! as another kind by the finder search on the frame as captured (`finder_candidates`).
//!
//! Frames are made on all cores but one and read on the main thread. The times of the decode
//! rate tables are taken while the other cores make frames; the time per frame of the timing
//! pass is taken with every frame made beforehand and nothing else running. A time covers
//! `nmtcode_detect::find` and `nmtcode::decode_with_erasures` on the decoded luma frame, not
//! JPEG decoding.

// A measurement program: counts and times become floating point for the report.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Instant;

use nmtcode::{DecodeOptions, decode_with_erasures};
use nmtcode_detect::{DetectOptions, LumaImage, Point, find, finder_candidates};
use nmtcode_sim::{Channel, Rng, canvas, random_symbol, render, seed};

/// A named condition.
#[derive(Clone)]
struct Condition {
    name: String,
    channel: Channel,
    sides: (u32, u32),
    levels: Vec<u8>,
    trials: usize,
    finder_stats: bool,
    /// Print each trial that did not decode.
    list: bool,
}

/// One made frame and what the trial needs to judge it; `image` is `None` when the symbol did
/// not fit in the frame.
struct Made {
    trial: usize,
    image: Option<LumaImage>,
    content: Vec<u8>,
    finder_centres: [Point; 4],
    pitch: f64,
}

#[derive(Default)]
struct Stats {
    trials: usize,
    decoded: usize,
    wrong: usize,
    no_fit: usize,
    times: Vec<f64>,
    /// Per finder TL, TR, BL, BR: not found, and found but classified as another kind.
    not_found: [usize; 4],
    misclassified: [usize; 4],
}

impl Stats {
    fn rate(&self) -> f64 {
        100.0 * self.decoded as f64 / (self.trials - self.no_fit).max(1) as f64
    }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn make(condition: &Condition, trial: usize) -> Made {
    let mut rng = Rng::new(seed(&condition.name, trial as u64));
    let steps = (condition.sides.1 - condition.sides.0) / 4 + 1;
    let w = condition.sides.0 + 4 * rng.below(u64::from(steps)) as u32;
    // Heights within a factor of 2 of the width, as the recommended size rule keeps them.
    let heights: Vec<u32> = (0..steps)
        .map(|i| condition.sides.0 + 4 * i)
        .filter(|&h| h <= 2 * w && w <= 2 * h)
        .collect();
    let h = heights[rng.below(heights.len() as u64) as usize];
    let level = condition.levels[rng.below(condition.levels.len() as u64) as usize];
    let empty = Made {
        trial,
        image: None,
        content: Vec::new(),
        finder_centres: [Point::default(); 4],
        pitch: 0.0,
    };
    let Some((symbol, content)) = random_symbol(w, h, level, &mut rng) else { return empty };
    let bootstrap = rng.chance(0.25);
    let Some(canvas) = canvas(&symbol, bootstrap) else { return empty };
    let Some(frame) = render(&canvas, &condition.channel, rng.next_u64()) else { return empty };
    Made {
        trial,
        image: Some(frame.image),
        content,
        finder_centres: frame.finder_centres,
        pitch: frame.min_pitch,
    }
}

/// Reads `image` and returns the time in milliseconds and the decoded contents.
fn read(image: &LumaImage) -> (f64, Vec<Vec<u8>>) {
    let start = Instant::now();
    let scan = find(image, &DetectOptions::default());
    let options = DecodeOptions::default();
    let contents = scan
        .found
        .iter()
        .filter_map(|f| decode_with_erasures(&f.grid, &f.uncertain, &options).ok())
        .map(|d| d.records.iter().flat_map(|r| r.value.iter().copied()).collect())
        .collect();
    (start.elapsed().as_secs_f64() * 1000.0, contents)
}

/// One line on a trial that did not decode.
fn describe(made: &Made, image: &LumaImage) -> String {
    let candidates = finder_candidates(image);
    let finders: String = made
        .finder_centres
        .iter()
        .enumerate()
        .map(|(kind, centre)| {
            let distance = |p: Point| (p.x - centre.x).hypot(p.y - centre.y);
            match candidates.iter().find(|c| distance(c.centre) <= 1.5 * made.pitch.max(1.0)) {
                None => '-',
                Some(c)
                    if (0..4).min_by(|&a, &b| c.scores[a].total_cmp(&c.scores[b]))
                        == Some(kind) =>
                {
                    'o'
                }
                Some(_) => 'm',
            }
        })
        .collect();
    let scan = find(image, &DetectOptions::default());
    let errors: Vec<&str> = scan.rejected.iter().map(|r| r.error.name()).collect();
    format!(
        "finders {finders}, found {}, rejected {errors:?}, pitch {:.2}",
        scan.found.len(),
        made.pitch
    )
}

fn judge(made: &Made, condition: &Condition, stats: &mut Stats) {
    stats.trials += 1;
    let Some(image) = &made.image else {
        stats.no_fit += 1;
        return;
    };
    let (ms, contents) = read(image);
    stats.times.push(ms);
    let good = contents.iter().filter(|c| **c == made.content).count();
    stats.wrong += contents.len() - good;
    if good > 0 {
        stats.decoded += 1;
    } else if condition.list {
        println!("  trial {} did not decode: {}", made.trial, describe(made, image));
    }
    if condition.finder_stats {
        let candidates = finder_candidates(image);
        for (kind, centre) in made.finder_centres.iter().enumerate() {
            let distance = |p: Point| (p.x - centre.x).hypot(p.y - centre.y);
            let near = candidates
                .iter()
                .filter(|c| distance(c.centre) <= 1.5 * made.pitch.max(1.0))
                .min_by(|a, b| distance(a.centre).total_cmp(&distance(b.centre)));
            match near {
                None => stats.not_found[kind] += 1,
                Some(c) => {
                    let best =
                        (0..4).min_by(|&a, &b| c.scores[a].total_cmp(&c.scores[b])).unwrap_or(0);
                    if best != kind {
                        stats.misclassified[kind] += 1;
                    }
                }
            }
        }
    }
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(2, std::num::NonZero::get).saturating_sub(1).max(1)
}

/// Runs every condition: frames on worker threads, the reader on this thread.
fn run(conditions: &[Condition]) -> Vec<Stats> {
    let jobs: Vec<(usize, usize)> = conditions
        .iter()
        .enumerate()
        .flat_map(|(c, cond)| (0..cond.trials).map(move |t| (c, t)))
        .collect();
    let mut stats: Vec<Stats> = conditions.iter().map(|_| Stats::default()).collect();
    let (tx, rx) = sync_channel::<(usize, Made)>(2 * workers());
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers() {
            let tx = tx.clone();
            let (jobs, next) = (&jobs, &next);
            scope.spawn(move || {
                while let Some(&(c, t)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if tx.send((c, make(&conditions[c], t))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        for (c, made) in rx {
            judge(&made, &conditions[c], &mut stats[c]);
        }
    });
    stats
}

/// Makes `n` frames of `condition` first, then reads them one by one with nothing else
/// running; returns the times in milliseconds.
fn timing(condition: &Condition, n: usize) -> Vec<f64> {
    let next = AtomicUsize::new(0);
    let frames: Vec<LumaImage> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers())
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut out = Vec::new();
                    loop {
                        let t = next.fetch_add(1, Ordering::Relaxed);
                        if t >= n {
                            break out;
                        }
                        if let Some(image) = make(condition, t).image {
                            out.push(image);
                        }
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    frames.iter().map(|image| read(image).0).collect()
}

fn args() -> (usize, usize, usize, String, bool) {
    let (mut a, mut s, mut t, mut only, mut list) = (500, 100, 200, String::from("all"), false);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    for pair in argv.chunks(2) {
        let value = pair.get(1).cloned().unwrap_or_default();
        match pair[0].as_str() {
            "--trials-a" => a = value.parse().unwrap_or(a),
            "--trials-sweep" => s = value.parse().unwrap_or(s),
            "--timing" => t = value.parse().unwrap_or(t),
            "--only" => only = value,
            "--list" => list = value == "1",
            _ => {}
        }
    }
    (a, s, t, only, list)
}

fn sweep(base: &Condition, trials: usize) {
    let mut conditions = Vec::new();
    for k in [2.0, 2.5, 3.0, 4.0] {
        for sigma in [0.3, 0.5, 0.7] {
            for tilt in [0.0, 30.0, 45.0] {
                let mut c = base.clone();
                c.name = format!("k{k}-s{sigma}-t{tilt}");
                c.channel.k = k;
                c.channel.blur = sigma;
                c.channel.tilt_deg = (tilt, tilt);
                c.trials = trials;
                c.finder_stats = k < 3.0;
                c.list = false;
                conditions.push(c);
            }
        }
    }
    let stats = run(&conditions);
    println!();
    println!(
        "sweep: decode rate (wrong data) over {trials} trials per cell; median ms per frame under load"
    );
    println!(
        "{:>4} {:>6} | {:>18} | {:>18} | {:>18}",
        "k", "sigma", "tilt 0", "tilt 30", "tilt 45"
    );
    for (row, chunk) in stats.chunks(3).enumerate() {
        let c = &conditions[row * 3];
        let cells: Vec<String> = chunk
            .iter()
            .map(|s| {
                format!("{:>6.1}% ({}) {:>5.1}ms", s.rate(), s.wrong, percentile(&s.times, 0.5))
            })
            .collect();
        println!(
            "{:>4} {:>6} | {:>18} | {:>18} | {:>18}",
            c.channel.k, c.channel.blur, cells[0], cells[1], cells[2]
        );
    }
    let no_fit: usize = stats.iter().map(|s| s.no_fit).sum();
    let wrong: usize = stats.iter().map(|s| s.wrong).sum();
    println!("frames that did not fit: {no_fit}; wrong data in the sweep: {wrong}");
    println!();
    println!(
        "finder failures at k = 2 and 2.5 (every sigma and tilt): not found / classified as another kind"
    );
    println!("{:>4} {:>14} {:>14} {:>14} {:>14}", "k", "TL", "TR", "BL", "BR");
    for k in [2.0, 2.5] {
        let (mut not_found, mut misclassified, mut frames) = ([0usize; 4], [0usize; 4], 0usize);
        for (c, s) in conditions.iter().zip(stats.iter()) {
            if (c.channel.k - k).abs() < 1e-9 {
                for i in 0..4 {
                    not_found[i] += s.not_found[i];
                    misclassified[i] += s.misclassified[i];
                }
                frames += s.trials - s.no_fit;
            }
        }
        let cells: Vec<String> =
            (0..4).map(|i| format!("{} / {}", not_found[i], misclassified[i])).collect();
        println!(
            "{:>4} {:>14} {:>14} {:>14} {:>14}   of {frames} frames",
            k, cells[0], cells[1], cells[2], cells[3]
        );
    }
}

fn main() {
    let (trials_a, trials_sweep, timing_frames, only, list) = args();
    let base = Condition {
        name: "A".to_owned(),
        channel: Channel::default(),
        sides: (24, 100),
        levels: vec![0, 1],
        trials: trials_a,
        finder_stats: false,
        list,
    };
    println!(
        "NMT Code reader on synthetic camera frames (nmtcode-sim; seeds fixed by condition and trial)"
    );
    if only == "all" || only == "a" {
        let stats = run(std::slice::from_ref(&base));
        let s = &stats[0];
        println!();
        println!(
            "condition A: 1920x1080, k = 3, sigma = 0.5 module, SNR 30 dB, JPEG 80, tilt 0-30 deg, any rotation, sides 24-100, levels 0 and 1"
        );
        println!(
            "{:<10} {:>6} {:>8} {:>8} {:>6} {:>7} {:>10} {:>8}",
            "condition", "trials", "decoded", "rate", "wrong", "no-fit", "median ms", "p95 ms"
        );
        println!(
            "{:<10} {:>6} {:>8} {:>7.2}% {:>6} {:>7} {:>10.2} {:>8.2}",
            "A",
            s.trials,
            s.decoded,
            s.rate(),
            s.wrong,
            s.no_fit,
            percentile(&s.times, 0.5),
            percentile(&s.times, 0.95)
        );
        let valid = s.trials - s.no_fit;
        let pass = s.decoded as f64 >= 0.99 * valid as f64 && s.wrong == 0 && valid >= 400;
        println!(
            "pass line (>= 99% of >= 400 trials, wrong 0): {}",
            if pass { "pass" } else { "fail" }
        );
    }
    if (only == "all" || only == "timing") && timing_frames > 0 {
        let times = timing(&base, timing_frames);
        println!();
        println!(
            "timing, condition A, {} frames read one at a time with nothing else running: median {:.2} ms, p95 {:.2} ms",
            times.len(),
            percentile(&times, 0.5),
            percentile(&times, 0.95)
        );
    }
    if only == "all" || only == "sweep" {
        sweep(&base, trials_sweep);
    }
}
