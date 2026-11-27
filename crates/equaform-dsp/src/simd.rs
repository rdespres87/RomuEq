//! SIMD-friendly stereo and mono Direct Form I biquad processing kernels.
//!
//! Uses fixed-width `[f32; 2]` stereo lanes (`StereoLane`) with hoisted
//! coefficients and register-resident delay lines so LLVM auto-vectorizes
//! stereo processing to NEON (`f32x2`) on ARM64 and SSE2 on x86_64 while
//! keeping `#![forbid(unsafe_code)]` intact.

use crate::coefficients::BiquadCoefficients;

/// A packed left/right sample pair for SIMD-friendly stereo processing.
#[derive(Clone, Copy, Debug)]
struct StereoLane([f32; 2]);

impl StereoLane {
    #[inline(always)]
    fn new(l: f32, r: f32) -> Self {
        Self([l, r])
    }

    #[inline(always)]
    fn left(self) -> f32 {
        self.0[0]
    }

    #[inline(always)]
    fn right(self) -> f32 {
        self.0[1]
    }

    #[inline(always)]
    fn mul_scalar(self, s: f32) -> Self {
        Self([self.0[0] * s, self.0[1] * s])
    }

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self([self.0[0] + rhs.0[0], self.0[1] + rhs.0[1]])
    }

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self([self.0[0] - rhs.0[0], self.0[1] - rhs.0[1]])
    }
}

/// Process a mono buffer in place using a Direct Form I biquad.
///
/// Delay state layout: `[x1, x2, y1, y2]`.
/// Coefficients are hoisted into locals; delay state is written back once after the loop.
#[inline]
pub fn process_mono_in_place(
    samples: &mut [f32],
    coeffs: &BiquadCoefficients,
    delay: &mut [f32; 4],
) {
    let b0 = coeffs.b0;
    let b1 = coeffs.b1;
    let b2 = coeffs.b2;
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;

    let mut x1 = delay[0];
    let mut x2 = delay[1];
    let mut y1 = delay[2];
    let mut y2 = delay[3];

    for sample in samples.iter_mut() {
        let x = *sample;
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        *sample = y;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    delay[0] = x1;
    delay[1] = x2;
    delay[2] = y1;
    delay[3] = y2;
}

/// Process planar stereo buffers in place using a Direct Form I biquad.
///
/// Each channel has its own delay state `[x1, x2, y1, y2]`.
/// Left and right are processed in parallel via `StereoLane`.
/// Processes `left.len().min(right.len())` frames without panicking on real-time threads.
#[inline]
pub fn process_stereo_planar_in_place(
    left: &mut [f32],
    right: &mut [f32],
    coeffs: &BiquadCoefficients,
    delay_l: &mut [f32; 4],
    delay_r: &mut [f32; 4],
) {
    let b0 = coeffs.b0;
    let b1 = coeffs.b1;
    let b2 = coeffs.b2;
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;

    let mut x1 = StereoLane::new(delay_l[0], delay_r[0]);
    let mut x2 = StereoLane::new(delay_l[1], delay_r[1]);
    let mut y1 = StereoLane::new(delay_l[2], delay_r[2]);
    let mut y2 = StereoLane::new(delay_l[3], delay_r[3]);

    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let x = StereoLane::new(*l, *r);
        let y = x
            .mul_scalar(b0)
            .add(x1.mul_scalar(b1))
            .add(x2.mul_scalar(b2))
            .sub(y1.mul_scalar(a1))
            .sub(y2.mul_scalar(a2));
        *l = y.left();
        *r = y.right();
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    delay_l[0] = x1.left();
    delay_l[1] = x2.left();
    delay_l[2] = y1.left();
    delay_l[3] = y2.left();
    delay_r[0] = x1.right();
    delay_r[1] = x2.right();
    delay_r[2] = y1.right();
    delay_r[3] = y2.right();
}

/// Process a mono buffer in place using a Direct Form I biquad with linear coefficient ramping.
#[inline]
pub fn process_mono_in_place_ramped(
    samples: &mut [f32],
    start: &BiquadCoefficients,
    target: &BiquadCoefficients,
    ramp_samples: usize,
    delay: &mut [f32; 4],
) {
    if samples.is_empty() {
        return;
    }

    let steps = ramp_samples.clamp(1, samples.len());
    let inv = 1.0 / steps as f32;

    let db0 = (target.b0 - start.b0) * inv;
    let db1 = (target.b1 - start.b1) * inv;
    let db2 = (target.b2 - start.b2) * inv;
    let da1 = (target.a1 - start.a1) * inv;
    let da2 = (target.a2 - start.a2) * inv;

    let mut x1 = delay[0];
    let mut x2 = delay[1];
    let mut y1 = delay[2];
    let mut y2 = delay[3];

    let mut b0 = start.b0;
    let mut b1 = start.b1;
    let mut b2 = start.b2;
    let mut a1 = start.a1;
    let mut a2 = start.a2;

    let (ramp_slice, tail_slice) = samples.split_at_mut(steps);
    for sample in ramp_slice {
        b0 += db0;
        b1 += db1;
        b2 += db2;
        a1 += da1;
        a2 += da2;
        let x = *sample;
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        *sample = y;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    let (b0, b1, b2, a1, a2) = (target.b0, target.b1, target.b2, target.a1, target.a2);
    for sample in tail_slice {
        let x = *sample;
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        *sample = y;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    delay[0] = x1;
    delay[1] = x2;
    delay[2] = y1;
    delay[3] = y2;
}

/// Linearly ramped biquad coefficient transition parameters.
#[derive(Debug, Clone, Copy)]
pub struct RampedBiquadCoeffs {
    pub start: BiquadCoefficients,
    pub target: BiquadCoefficients,
    pub ramp_samples: usize,
}

/// Process planar stereo buffers in place using a Direct Form I biquad with linear coefficient ramping.
#[inline]
pub fn process_stereo_planar_in_place_ramped(
    left: &mut [f32],
    right: &mut [f32],
    ramp: RampedBiquadCoeffs,
    delay_l: &mut [f32; 4],
    delay_r: &mut [f32; 4],
) {
    let len = left.len().min(right.len());
    if len == 0 {
        return;
    }
    let steps = ramp.ramp_samples.clamp(1, len);
    let inv = 1.0 / steps as f32;
    let start = &ramp.start;
    let target = &ramp.target;

    let mut dl = StereoLane::new(delay_l[0], delay_r[0]);
    let mut dl2 = StereoLane::new(delay_l[1], delay_r[1]);
    let mut dy = StereoLane::new(delay_l[2], delay_r[2]);
    let mut dy2 = StereoLane::new(delay_l[3], delay_r[3]);

    for (i, (l, r)) in left[..len]
        .iter_mut()
        .zip(right[..len].iter_mut())
        .enumerate()
    {
        let t = if i < steps {
            (i as f32 + 1.0) * inv
        } else {
            1.0
        };

        let b0 = start.b0 + (target.b0 - start.b0) * t;
        let b1 = start.b1 + (target.b1 - start.b1) * t;
        let b2 = start.b2 + (target.b2 - start.b2) * t;
        let a1 = start.a1 + (target.a1 - start.a1) * t;
        let a2 = start.a2 + (target.a2 - start.a2) * t;

        let x = StereoLane::new(*l, *r);
        let y = x
            .mul_scalar(b0)
            .add(dl.mul_scalar(b1))
            .add(dl2.mul_scalar(b2))
            .sub(dy.mul_scalar(a1))
            .sub(dy2.mul_scalar(a2));

        *l = y.left();
        *r = y.right();

        dl2 = dl;
        dl = x;
        dy2 = dy;
        dy = y;
    }

    delay_l[0] = dl.left();
    delay_l[1] = dl2.left();
    delay_l[2] = dy.left();
    delay_l[3] = dy2.left();
    delay_r[0] = dl.right();
    delay_r[1] = dl2.right();
    delay_r[2] = dy.right();
    delay_r[3] = dy2.right();
}

/// Process interleaved stereo input into interleaved output using a Direct Form I biquad.
///
/// Input and output are `[L0, R0, L1, R1, ...]`.
/// Each channel has its own delay state `[x1, x2, y1, y2]`.
/// Processes `input.len().min(output.len()) / 2` complete stereo frames without panicking;
/// any trailing unpaired sample is left untouched (`BiquadFilter::process` enforces even lengths).
#[inline]
pub fn process_stereo_interleaved(
    input: &[f32],
    output: &mut [f32],
    coeffs: &BiquadCoefficients,
    delay_l: &mut [f32; 4],
    delay_r: &mut [f32; 4],
) {
    let b0 = coeffs.b0;
    let b1 = coeffs.b1;
    let b2 = coeffs.b2;
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;

    let mut x1 = StereoLane::new(delay_l[0], delay_r[0]);
    let mut x2 = StereoLane::new(delay_l[1], delay_r[1]);
    let mut y1 = StereoLane::new(delay_l[2], delay_r[2]);
    let mut y2 = StereoLane::new(delay_l[3], delay_r[3]);

    for (in_pair, out_pair) in input.chunks_exact(2).zip(output.chunks_exact_mut(2)) {
        let x = StereoLane::new(in_pair[0], in_pair[1]);
        let y = x
            .mul_scalar(b0)
            .add(x1.mul_scalar(b1))
            .add(x2.mul_scalar(b2))
            .sub(y1.mul_scalar(a1))
            .sub(y2.mul_scalar(a2));
        out_pair[0] = y.left();
        out_pair[1] = y.right();
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    delay_l[0] = x1.left();
    delay_l[1] = x2.left();
    delay_l[2] = y1.left();
    delay_l[3] = y2.left();
    delay_r[0] = x1.right();
    delay_r[1] = x2.right();
    delay_r[2] = y1.right();
    delay_r[3] = y2.right();
}

/// Convert planar Left/Right buffers to orthonormal Mid/Side in-place.
///
/// Uses `FRAC_1_SQRT_2` scaling (`M = (L + R) / sqrt(2)`, `S = (L - R) / sqrt(2)`)
/// so [`lr_to_ms_in_place`] followed by [`ms_to_lr_in_place`] is unity-gain exact.
/// Processes `left.len().min(right.len())` samples without allocating or panicking.
#[inline]
pub fn lr_to_ms_in_place(left: &mut [f32], right: &mut [f32]) {
    const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let lv = *l;
        let rv = *r;
        *l = (lv + rv) * INV_SQRT_2;
        *r = (lv - rv) * INV_SQRT_2;
    }
}

/// Convert planar orthonormal Mid/Side buffers back to Left/Right in-place.
///
/// Uses `FRAC_1_SQRT_2` scaling (`L = (M + S) / sqrt(2)`, `R = (M - S) / sqrt(2)`).
/// Processes `mid.len().min(side.len())` samples without allocating or panicking.
#[inline]
pub fn ms_to_lr_in_place(mid: &mut [f32], side: &mut [f32]) {
    const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    for (m, s) in mid.iter_mut().zip(side.iter_mut()) {
        let mv = *m;
        let sv = *s;
        *m = (mv + sv) * INV_SQRT_2;
        *s = (mv - sv) * INV_SQRT_2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coefficients::compute_coefficients;
    use crate::filter::BiquadState;
    use equaform_common::{FilterType, Slope};

    const SR: f32 = 48_000.0;
    const FREQ: f32 = 1_000.0;
    const GAIN_DB: f32 = 4.5;
    const Q: f32 = 0.707;

    fn coeffs(ft: FilterType) -> BiquadCoefficients {
        compute_coefficients(ft, FREQ, GAIN_DB, Q, Slope::Db12, SR)
    }

    fn all_types() -> [FilterType; 5] {
        [
            FilterType::Bell,
            FilterType::LowPass,
            FilterType::HighPass,
            FilterType::LowShelf,
            FilterType::HighShelf,
        ]
    }

    fn sine_impulse(n: usize, phase: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / SR;
                let s = (2.0 * std::f32::consts::PI * FREQ * t + phase).sin();
                s + if i == 0 { 1.0 } else { 0.0 }
            })
            .collect()
    }

    fn max_err(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max)
    }

    #[test]
    fn test_mono_matches_scalar_reference() {
        for ft in all_types() {
            let c = coeffs(ft);
            let mut sig = sine_impulse(256, 0.0);
            let mut delay = [0.0f32; 4];
            process_mono_in_place(&mut sig, &c, &mut delay);

            let mut ref_sig = sine_impulse(256, 0.0);
            let mut st = BiquadState::new(c);
            for s in ref_sig.iter_mut() {
                *s = st.process(*s);
            }
            assert!(max_err(&sig, &ref_sig) < 1e-5, "mono {ft:?}");
        }
    }

    #[test]
    fn test_stereo_planar_matches_scalar_reference() {
        for ft in all_types() {
            let c = coeffs(ft);
            let mut l = sine_impulse(256, 0.0);
            let mut r = sine_impulse(256, std::f32::consts::PI / 3.0);
            let mut dl = [0.0f32; 4];
            let mut dr = [0.0f32; 4];
            process_stereo_planar_in_place(&mut l, &mut r, &c, &mut dl, &mut dr);

            let mut rl = sine_impulse(256, 0.0);
            let mut rr = sine_impulse(256, std::f32::consts::PI / 3.0);
            let mut sl = BiquadState::new(c);
            let mut sr = BiquadState::new(c);
            for (lv, rv) in rl.iter_mut().zip(rr.iter_mut()) {
                *lv = sl.process(*lv);
                *rv = sr.process(*rv);
            }
            assert!(max_err(&l, &rl) < 1e-5, "planar L {ft:?}");
            assert!(max_err(&r, &rr) < 1e-5, "planar R {ft:?}");
        }
    }

    #[test]
    fn test_stereo_interleaved_matches_scalar_reference() {
        for ft in all_types() {
            let c = coeffs(ft);
            let l = sine_impulse(128, 0.0);
            let r = sine_impulse(128, std::f32::consts::PI / 4.0);
            let mut input = Vec::with_capacity(256);
            for (&lv, &rv) in l.iter().zip(r.iter()) {
                input.push(lv);
                input.push(rv);
            }
            let mut output = vec![0.0f32; 256];
            let mut dl = [0.0f32; 4];
            let mut dr = [0.0f32; 4];
            process_stereo_interleaved(&input, &mut output, &c, &mut dl, &mut dr);

            let mut sl = BiquadState::new(c);
            let mut sr = BiquadState::new(c);
            for ((out_pair, &lv), &rv) in output.chunks_exact(2).zip(l.iter()).zip(r.iter()) {
                let ref_l = sl.process(lv);
                let ref_r = sr.process(rv);
                assert!((out_pair[0] - ref_l).abs() < 1e-5, "interleaved L {ft:?}");
                assert!((out_pair[1] - ref_r).abs() < 1e-5, "interleaved R {ft:?}");
            }
        }
    }

    #[test]
    fn test_stereo_planar_multi_block_continuity() {
        let c = coeffs(FilterType::Bell);
        let l = sine_impulse(512, 0.0);
        let r = sine_impulse(512, 1.0);

        let mut single_l = l.clone();
        let mut single_r = r.clone();
        let mut dl = [0.0f32; 4];
        let mut dr = [0.0f32; 4];
        process_stereo_planar_in_place(&mut single_l, &mut single_r, &c, &mut dl, &mut dr);

        let mut block_l = l;
        let mut block_r = r;
        let mut dl2 = [0.0f32; 4];
        let mut dr2 = [0.0f32; 4];
        for start in (0..512).step_by(128) {
            let end = start + 128;
            process_stereo_planar_in_place(
                &mut block_l[start..end],
                &mut block_r[start..end],
                &c,
                &mut dl2,
                &mut dr2,
            );
        }
        assert!(max_err(&single_l, &block_l) < 1e-5, "L continuity");
        assert!(max_err(&single_r, &block_r) < 1e-5, "R continuity");
    }

    #[test]
    fn test_empty_and_mismatched_lengths_do_not_panic() {
        let c = coeffs(FilterType::LowPass);
        let mut dl = [0.0f32; 4];
        let mut dr = [0.0f32; 4];

        let mut empty: [f32; 0] = [];
        process_mono_in_place(&mut empty, &c, &mut dl);

        let mut l: Vec<f32> = vec![];
        let mut r: Vec<f32> = vec![];
        process_stereo_planar_in_place(&mut l, &mut r, &c, &mut dl, &mut dr);

        let odd_input = [1.0, 2.0, 3.0];
        let mut odd_out = [0.0f32; 3];
        process_stereo_interleaved(&odd_input, &mut odd_out, &c, &mut dl, &mut dr);
        assert_eq!(odd_out[2], 0.0);

        let mut l2 = [1.0, 2.0, 3.0];
        let mut r2 = [1.0];
        process_stereo_planar_in_place(&mut l2, &mut r2, &c, &mut dl, &mut dr);
        assert_eq!(l2[1], 2.0);
        assert_eq!(l2[2], 3.0);
    }

    #[test]
    fn test_lr_ms_roundtrip_unity_gain() {
        let mut l = [1.0f32, -0.5, 0.3, 2.0];
        let mut r = [0.7f32, 1.1, -0.2, -1.5];
        let (orig_l, orig_r) = (l, r);
        lr_to_ms_in_place(&mut l, &mut r);
        ms_to_lr_in_place(&mut l, &mut r);
        for (&got, &exp) in l.iter().zip(orig_l.iter()) {
            assert!((got - exp).abs() < 1e-6);
        }
        for (&got, &exp) in r.iter().zip(orig_r.iter()) {
            assert!((got - exp).abs() < 1e-6);
        }
    }

    #[test]
    fn test_lr_to_ms_mono_and_antiphase() {
        let mut l = [1.0f32, -1.0];
        let mut r = [1.0f32, 1.0];
        lr_to_ms_in_place(&mut l, &mut r);
        assert!((l[0] - std::f32::consts::SQRT_2).abs() < 1e-6);
        assert!(r[0].abs() < 1e-6);
        assert!(l[1].abs() < 1e-6);
        assert!((r[1] + std::f32::consts::SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn test_lr_ms_empty_and_mismatched_safe() {
        let mut l = [1.0f32, 2.0, 3.0];
        let mut r = [4.0f32, 5.0];
        lr_to_ms_in_place(&mut l, &mut r);
        assert!((l[2] - 3.0).abs() < 1e-6);
        let mut m = [1.0f32, 2.0, 3.0];
        let mut s = [4.0f32, 5.0];
        ms_to_lr_in_place(&mut m, &mut s);
        assert!((m[2] - 3.0).abs() < 1e-6);
        let mut e1: [f32; 0] = [];
        let mut e2: [f32; 0] = [];
        lr_to_ms_in_place(&mut e1, &mut e2);
        ms_to_lr_in_place(&mut e1, &mut e2);
    }
}
