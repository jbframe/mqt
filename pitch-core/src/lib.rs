//! Pitch detection core for the Karaoke Practice Site pitch check.
//!
//! Dependency-free autocorrelation pitch tracker with a plain C ABI so the
//! page's classic (non-module) JavaScript can drive it directly:
//!
//!   let ptr = alloc(n)                 // reserve n f32 samples
//!   memory.set(samples, ptr / 4)      // copy mic samples in
//!   let hz  = detect_pitch(ptr, n, sample_rate)
//!   let conf = last_clarity()          // 0.0 .. 1.0 confidence
//!   dealloc(ptr, n)
//!
//! `detect_pitch` returns the fundamental frequency in Hz, or -1.0 when the
//! frame is silence / noise with no confident periodic component.

use std::cell::Cell;

thread_local! {
    static CLARITY: Cell<f32> = const { Cell::new(0.0) };
}

/// Lowest / highest fundamental we accept for a singing voice (Hz).
const MIN_HZ: f32 = 60.0;
const MAX_HZ: f32 = 1100.0;
/// Frames quieter than this RMS are treated as silence.
const RMS_GATE: f32 = 0.012;
/// Minimum normalized autocorrelation peak accepted as a real pitch.
const CLARITY_GATE: f32 = 0.55;

/// Reserve space for `len` f32 samples; returns a byte offset into memory.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut f32 {
    let mut buf = Vec::<f32>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Release a buffer previously returned by `alloc` with the same `len`.
#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut f32, len: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

/// Confidence (normalized autocorrelation peak height) of the last
/// `detect_pitch` call: 0.0 = noise/silence, 1.0 = perfectly periodic.
#[no_mangle]
pub extern "C" fn last_clarity() -> f32 {
    CLARITY.with(|c| c.get())
}

/// Detect the fundamental frequency of one frame of mono f32 samples.
/// Returns Hz, or -1.0 when no confident pitch is present.
#[no_mangle]
pub extern "C" fn detect_pitch(ptr: *const f32, len: usize, sample_rate: f32) -> f32 {
    CLARITY.with(|c| c.set(0.0));
    if ptr.is_null() || len < 256 || sample_rate <= 0.0 {
        return -1.0;
    }
    let samples = unsafe { std::slice::from_raw_parts(ptr, len) };

    // RMS gate: don't chase pitch in silence or room noise floor.
    let mut energy = 0.0f32;
    for &s in samples {
        energy += s * s;
    }
    let rms = (energy / len as f32).sqrt();
    if rms < RMS_GATE {
        return -1.0;
    }

    let min_period = (sample_rate / MAX_HZ).floor() as usize;
    let max_period = ((sample_rate / MIN_HZ).ceil() as usize).min(len / 2);
    if max_period <= min_period + 2 {
        return -1.0;
    }

    // Normalized autocorrelation over the plausible period range.
    let n = len;
    let mut best_period = 0usize;
    let mut best_corr = f32::MIN;
    let mut prev_corr = 0.0f32;
    let mut rising_past = false;
    for period in min_period..=max_period {
        let mut num = 0.0f32;
        let mut den_a = 0.0f32;
        let mut den_b = 0.0f32;
        let limit = n - period;
        for i in 0..limit {
            let a = samples[i];
            let b = samples[i + period];
            num += a * b;
            den_a += a * a;
            den_b += b * b;
        }
        let denom = (den_a * den_b).sqrt();
        let corr = if denom > 0.0 { num / denom } else { 0.0 };
        // Take the first strong local maximum: later peaks are usually
        // period multiples (octave-down traps).
        if corr > 0.9 && prev_corr <= corr {
            rising_past = true;
        } else if rising_past && corr < prev_corr {
            best_period = period - 1;
            best_corr = prev_corr;
            break;
        }
        if corr > best_corr {
            best_corr = corr;
            best_period = period;
        }
        prev_corr = corr;
    }
    if best_period == 0 || best_corr < CLARITY_GATE {
        return -1.0;
    }

    // Parabolic interpolation around the peak for sub-sample accuracy.
    let corr_at = |p: usize| -> f32 {
        if p == 0 || p >= n / 2 {
            return 0.0;
        }
        let mut num = 0.0f32;
        let mut den_a = 0.0f32;
        let mut den_b = 0.0f32;
        let limit = n - p;
        for i in 0..limit {
            let a = samples[i];
            let b = samples[i + p];
            num += a * b;
            den_a += a * a;
            den_b += b * b;
        }
        let denom = (den_a * den_b).sqrt();
        if denom > 0.0 { num / denom } else { 0.0 }
    };
    let y0 = corr_at(best_period.saturating_sub(1));
    let y1 = best_corr;
    let y2 = corr_at(best_period + 1);
    let denom = y0 - 2.0 * y1 + y2;
    let shift = if denom.abs() > 1e-9 {
        0.5 * (y0 - y2) / denom
    } else {
        0.0
    };
    let period = best_period as f32 + shift.clamp(-1.0, 1.0);
    if period <= 0.0 {
        return -1.0;
    }
    CLARITY.with(|c| c.set(best_corr.clamp(0.0, 1.0)));
    sample_rate / period
}
