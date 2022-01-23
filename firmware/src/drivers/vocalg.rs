// taken from here: https://github.com/mjaakkol/sgp40-rs

/// This work is a port of Sensirion VOC indexing algorithm from
/// https://github.com/Sensirion/embedded-sgp/tree/master/sgp40_voc_index
/*
 * Copyright (c) 2020, Sensirion AG
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 * * Redistributions of source code must retain the above copyright notice, this
 *   list of conditions and the following disclaimer.
 *
 * * Redistributions in binary form must reproduce the above copyright notice,
 *   this list of conditions and the following disclaimer in the documentation
 *   and/or other materials provided with the distribution.
 *
 * * Neither the name of Sensirion AG nor the names of its
 *   contributors may be used to endorse or promote products derived from
 *   this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
 * LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */
use fixed::{consts::E, traits::FromFixed, types::I16F16};
use fixed_sqrt::FixedSqrt;

type Fix = I16F16;

//use fixed_macro::fixed;
//macro_rules! alg_fixed {
//    ($a:expr) => {{
//        fixed!($a: I16F16)
//    }};

macro_rules! alg_fixed {
    ($a:expr) => {{
        Fix::from_num($a)
    }};
}

const ZERO: Fix = Fix::from_bits(0x0000_0000); // 1
const SAMPLING_INTERVAL: Fix = Fix::from_bits(0x0001_0000); // 1
const INITIAL_BLACKOUT: Fix = Fix::from_bits(0x002D_0000); // 45
const VOC_INDEX_GAIN: Fix = Fix::from_bits(0x00E6_0000); // 230
const SRAW_STD_INITIAL: Fix = Fix::from_bits(0x0032_0000); //50
const SRAW_STD_BONUS: Fix = Fix::from_bits(0x00DC_0000); //220
const TAU_MEAN_VARIANCE_HOURS: Fix = Fix::from_bits(0x000C_0000); // 12
const TAU_INITIAL_MEAN: Fix = Fix::from_bits(0x0014_0000); // 20
const INIT_DURATION_MEAN: Fix = Fix::from_bits(0x0ABC_0000); // 3600. * 0.75
const INIT_TRANSITION_MEAN: Fix = Fix::from_bits(0x0000_028F); // 0.01
const TAU_INITIAL_VARIANCE: Fix = Fix::from_bits(0x09C4_0000); // 2500
const INIT_DURATION_VARIANCE: Fix = Fix::from_bits(0x1450_0000); // 3600. * 1.45
const INIT_TRANSITION_VARIANCE: Fix = Fix::from_bits(0x0000_028F); // 0.01
const GATING_THRESHOLD: Fix = Fix::from_bits(0x0154_0000); // 340
const GATING_THRESHOLD_INITIAL: Fix = Fix::from_bits(0x01FE_0000); // 510
const GATING_THRESHOLD_TRANSITION: Fix = Fix::from_bits(0x0000_170A); // 0.09
const GATING_MAX_DURATION_MINUTES: Fix = Fix::from_bits(0x00B4_0000); // 60.0 * 3.0
const GATING_MAX_RATIO: Fix = Fix::from_bits(0x0000_4CCD); // 0.3
const SIGMOID_L: Fix = Fix::from_bits(0x01F4_0000); // 500
                                                    //const SIGMOID_K: Fix = Fix::from_bits(0xFFFF_FE56); // -0.0065
const SIGMOID_X0: Fix = Fix::from_bits(0x00D5_0000); // 213
const VOC_INDEX_OFFSET_DEFAULT: Fix = Fix::from_bits(0x0064_0000); // 100
const LP_TAU_FAST: Fix = Fix::from_bits(0x0014_0000); // 20
const LP_TAU_SLOW: Fix = Fix::from_bits(0x01F4_0000); // 500
                                                      //const LP_ALPHA: Fix = Fix::from_bits(0xFFFF_CCCD); // -0.2
const PERSISTENCE_UPTIME_GAMMA: Fix = Fix::from_bits(0x2A30_0000); // 500 // 3 * 3600
const MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING: Fix = Fix::from_bits(0x0040_0000);
const MEAN_VARIANCE_ESTIMATOR_FIX16_MAX: Fix = Fix::from_bits(0x7FFF_0000);

// Stores VOC algorithm states
#[allow(dead_code)]
pub struct VocAlgorithm {
    voc_index_offset: Fix,
    tau_mean_variance_hours: Fix,
    gating_max_duration_minutes: Fix,
    sraw_std_initial: Fix,
    uptime: Fix,
    sraw: Fix,
    voc_index: Fix,
    mean_variance_estimator: MeanVarianceEstimator,
    mox_model: MoxModel,
    sigmoid_scaled: SigmoidScaledInit,
    adaptive_lowpass: AdaptiveLowpass,
}

impl Default for VocAlgorithm {
    fn default() -> Self {
        let (mean_variance_estimator, mox_model, sigmoid_scaled, adaptive_lowpass) =
            VocAlgorithm::new_instances(
                SRAW_STD_INITIAL,
                TAU_MEAN_VARIANCE_HOURS,
                GATING_MAX_DURATION_MINUTES,
                VOC_INDEX_OFFSET_DEFAULT,
            );

        Self {
            voc_index_offset: VOC_INDEX_OFFSET_DEFAULT,
            tau_mean_variance_hours: TAU_MEAN_VARIANCE_HOURS,
            gating_max_duration_minutes: GATING_MAX_DURATION_MINUTES,
            sraw_std_initial: SRAW_STD_INITIAL,
            uptime: ZERO,
            sraw: ZERO,
            voc_index: ZERO,
            mean_variance_estimator,
            mox_model,
            sigmoid_scaled,
            adaptive_lowpass,
        }
    }
}

impl VocAlgorithm {
    fn new_instances(
        sraw_std_initial: Fix,
        tau_mean_variance_hours: Fix,
        gating_max_duration_minutes: Fix,
        voc_index_offset: Fix,
    ) -> (
        MeanVarianceEstimator,
        MoxModel,
        SigmoidScaledInit,
        AdaptiveLowpass,
    ) {
        let mut mean_variance_estimator = MeanVarianceEstimator::new();
        mean_variance_estimator.set_parameters(
            sraw_std_initial,
            tau_mean_variance_hours,
            gating_max_duration_minutes,
        );

        let mox_model = MoxModel::new(
            mean_variance_estimator.get_std(),
            mean_variance_estimator.get_mean(),
        );

        let sigmoid_scaled = SigmoidScaledInit::new(voc_index_offset);

        let adaptive_lowpass = AdaptiveLowpass::new();

        (
            mean_variance_estimator,
            mox_model,
            sigmoid_scaled,
            adaptive_lowpass,
        )
    }

    /// Returns state0, state1
    pub fn get_states(&self) -> (i32, i32) {
        (
            i32::saturating_from_fixed(self.mean_variance_estimator.get_mean()),
            i32::saturating_from_fixed(self.mean_variance_estimator.get_std()),
        )
    }

    /// Used for setting state0 and state1 to return to the previously calibrated states.
    pub fn set_states(&mut self, state0: i32, state1: i32) {
        self.mean_variance_estimator.set_states(
            Fix::from_num(state0),
            Fix::from_num(state1),
            PERSISTENCE_UPTIME_GAMMA,
        );

        self.sraw = Fix::from_num(state0);
    }

    pub fn set_tuning_parameters(
        &mut self,
        voc_index_offset: i32,
        learning_time_hours: i32,
        gating_duration_minutes: i32,
        std_initial: i32,
    ) {
        let (mean_variance_estimator, mox_model, sigmoid_scaled, adaptive_lowpass) =
            VocAlgorithm::new_instances(
                Fix::from_num(std_initial),
                Fix::from_num(learning_time_hours),
                Fix::from_num(gating_duration_minutes),
                Fix::from_num(voc_index_offset),
            );

        self.mean_variance_estimator = mean_variance_estimator;
        self.mox_model = mox_model;
        self.sigmoid_scaled = sigmoid_scaled;
        self.adaptive_lowpass = adaptive_lowpass;
    }

    pub fn process(&mut self, sraw: i32) -> i32 {
        if self.uptime <= INITIAL_BLACKOUT {
            self.uptime += SAMPLING_INTERVAL;
        } else {
            assert!(sraw > 0 && sraw < 65000);

            // -20000 from all the numbers
            let sraw = if sraw < 20001 {
                1
            } else if sraw > 52767 {
                32767
            } else {
                sraw - 20000
            };

            self.sraw = Fix::from_num(sraw);

            //println!("SRAW: {}", self.sraw);

            self.voc_index = self.mox_model.process(self.sraw);
            //println!("After Mox: {}", self.voc_index);

            self.voc_index = self.sigmoid_scaled.process(self.voc_index);
            //println!("After Sigmoid scaled: {}", self.voc_index);

            self.voc_index = self.adaptive_lowpass.process(self.voc_index);
            //println!("After Adaptive lowpass: {}", self.voc_index);

            if self.voc_index < alg_fixed!(0.5) {
                self.voc_index = alg_fixed!(0.5);
            }

            if self.sraw > alg_fixed!(0) {
                self.mean_variance_estimator
                    .process(self.sraw, self.voc_index);
                //println!("Est std:{} mean:{}", self.mean_variance_estimator.get_std(),self.mean_variance_estimator.get_mean());
                self.mox_model = MoxModel::new(
                    self.mean_variance_estimator.get_std(),
                    self.mean_variance_estimator.get_mean(),
                );
            }
        }
        i32::saturating_from_fixed(self.voc_index + alg_fixed!(0.5))
    }
}

struct MeanVarianceEstimator {
    gating_max_duration_minutes: Fix,
    mean: Fix,
    sraw_offset: Fix,
    std: Fix,
    gamma: Fix,
    gamma_initial_mean: Fix,
    gamma_initial_variance: Fix,
    gamma_mean: Fix,
    gamma_variance: Fix,
    uptime_gamma: Fix,
    uptime_gating: Fix,
    gating_duration_minutes: Fix,
    sigmoid: MeanVarianceEstimatorSigmoid,
    initialized: bool,
}

impl MeanVarianceEstimator {
    fn new() -> Self {
        MeanVarianceEstimator {
            gating_max_duration_minutes: ZERO,
            mean: ZERO,
            sraw_offset: ZERO,
            std: ZERO,
            gamma: ZERO,
            gamma_initial_mean: ZERO,
            gamma_initial_variance: ZERO,
            gamma_mean: ZERO,
            gamma_variance: ZERO,
            uptime_gamma: ZERO,
            uptime_gating: ZERO,
            gating_duration_minutes: ZERO,
            sigmoid: MeanVarianceEstimatorSigmoid::new(),
            initialized: false,
        }
    }

    fn set_parameters(
        &mut self,
        std_initial: Fix,
        tau_mean_variance_hours: Fix,
        gating_max_duration_minutes: Fix,
    ) {
        self.gating_max_duration_minutes = gating_max_duration_minutes;
        self.initialized = false;
        self.mean = ZERO;
        self.sraw_offset = ZERO;
        self.std = std_initial;
        self.gamma = (MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING
            * (SAMPLING_INTERVAL / alg_fixed!(3600)))
            / (tau_mean_variance_hours + SAMPLING_INTERVAL / alg_fixed!(3600));
        self.gamma_initial_mean = (MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING * SAMPLING_INTERVAL)
            / (TAU_INITIAL_MEAN + SAMPLING_INTERVAL);
        self.gamma_initial_variance = (MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING * SAMPLING_INTERVAL)
            / (TAU_INITIAL_VARIANCE + SAMPLING_INTERVAL);
        self.gamma_mean = ZERO;
        self.gamma_variance = ZERO;
        self.uptime_gamma = ZERO;
        self.uptime_gating = ZERO;
        self.gating_duration_minutes = ZERO;
    }

    fn set_states(&mut self, mean: Fix, std: Fix, uptime_gamma: Fix) {
        self.mean = mean;
        self.std = std;
        self.uptime_gamma = uptime_gamma;
        self.initialized = true;
    }

    fn get_std(&self) -> Fix {
        self.std
    }

    fn get_mean(&self) -> Fix {
        self.mean + self.sraw_offset
    }

    fn calculate_gamma(&mut self, voc_index_from_prior: Fix) {
        // Check this as we are likely running in 32-bit environment
        let uptime_limit = MEAN_VARIANCE_ESTIMATOR_FIX16_MAX - SAMPLING_INTERVAL;

        //println!("Updatime gamma:{}", self.uptime_gamma);

        if self.uptime_gamma < uptime_limit {
            self.uptime_gamma += SAMPLING_INTERVAL;
        }

        if self.uptime_gating < uptime_limit {
            self.uptime_gating += SAMPLING_INTERVAL;
        }

        self.sigmoid
            .set_parameters(alg_fixed!(1), INIT_DURATION_MEAN, INIT_TRANSITION_MEAN);

        let sigmoid_gamma_mean = self.sigmoid.process(self.uptime_gamma);

        let gamma_mean = self.gamma + ((self.gamma_initial_mean - self.gamma) * sigmoid_gamma_mean);

        let gating_threshold_mean = GATING_THRESHOLD
            + (GATING_THRESHOLD_INITIAL - GATING_THRESHOLD)
                * self.sigmoid.process(self.uptime_gating);

        self.sigmoid.set_parameters(
            alg_fixed!(1),
            gating_threshold_mean,
            GATING_THRESHOLD_TRANSITION,
        );

        let sigmoid_gating_mean = self.sigmoid.process(voc_index_from_prior);

        self.gamma_mean = sigmoid_gating_mean * gamma_mean;

        self.sigmoid.set_parameters(
            alg_fixed!(1),
            INIT_DURATION_VARIANCE,
            INIT_TRANSITION_VARIANCE,
        );

        let sigmoid_gamma_variance = self.sigmoid.process(self.uptime_gamma);

        let gamma_variance = self.gamma
            + (self.gamma_initial_variance - self.gamma)
                * (sigmoid_gamma_variance - sigmoid_gamma_mean);

        let gating_threshold_variance = GATING_THRESHOLD
            + (GATING_THRESHOLD_INITIAL - GATING_THRESHOLD)
                * self.sigmoid.process(self.uptime_gating);

        self.sigmoid.set_parameters(
            alg_fixed!(1),
            gating_threshold_variance,
            GATING_THRESHOLD_TRANSITION,
        );

        let sigmoid_gating_variance = self.sigmoid.process(voc_index_from_prior);

        self.gamma_variance = sigmoid_gating_variance * gamma_variance;

        self.gating_duration_minutes += (SAMPLING_INTERVAL / alg_fixed!(60))
            * (((alg_fixed!(1) - sigmoid_gating_mean) * (alg_fixed!(1) + GATING_MAX_RATIO))
                - GATING_MAX_RATIO);

        if self.gating_duration_minutes < ZERO {
            self.gating_duration_minutes = ZERO;
        }

        if self.gating_duration_minutes > self.gating_max_duration_minutes {
            self.uptime_gating = ZERO;
        }
    }

    fn process(&mut self, sraw: Fix, voc_index_from_prior: Fix) {
        if !self.initialized {
            self.initialized = true;
            self.sraw_offset = sraw;
            self.mean = ZERO;
        } else {
            if self.mean >= alg_fixed!(100) || self.mean <= alg_fixed!(-100) {
                self.sraw_offset += self.mean;
                self.mean = ZERO;
                //println!("Mean reset");
            }

            let sraw = sraw - self.sraw_offset;

            self.calculate_gamma(voc_index_from_prior);

            //println!("Gamma variance:{}", self.gamma_variance);

            let delta_sgp = (sraw - self.mean) / MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING;

            let c = self.std + delta_sgp.abs();

            let additional_scaling = if c > alg_fixed!(1440) {
                alg_fixed!(4)
            } else {
                alg_fixed!(1)
            };

            self.std = (additional_scaling
                * (MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING - self.gamma_variance))
                .sqrt()
                * ((self.std
                    * (self.std / (MEAN_VARIANCE_ESTIMATOR_GAMMA_SCALING * additional_scaling)))
                    + (((self.gamma_variance * delta_sgp) / additional_scaling) * delta_sgp))
                    .sqrt();

            self.mean += self.gamma_mean * delta_sgp;
            //println!("Final mean:{} std:{}", self.mean, self.std);
        }
    }
}

#[allow(non_snake_case)]
struct MeanVarianceEstimatorSigmoid {
    L: Fix,
    K: Fix,
    X0: Fix,
}

impl MeanVarianceEstimatorSigmoid {
    fn new() -> Self {
        MeanVarianceEstimatorSigmoid {
            L: ZERO,
            K: ZERO,
            X0: ZERO,
        }
    }

    fn process(&self, sample: Fix) -> Fix {
        let (x, b) = self.K.overflowing_mul(sample - self.X0);

        //println!("Sigmoid: sample:{} x:{}", sample, x);
        if b {
            return ZERO;
        }

        if x < alg_fixed!(-50) {
            self.L
        } else if x > alg_fixed!(50) {
            ZERO
        } else {
            self.L / (alg_fixed!(1) + fixed_exp(x))
        }
    }

    #[allow(non_snake_case)]
    fn set_parameters(&mut self, L: Fix, K: Fix, X0: Fix) {
        self.L = L;
        self.K = K;
        self.X0 = X0;
    }
}

// Needs new implementation as the original code prevents going above 16-bit ranges
fn fixed_exp(x: Fix) -> Fix {
    let exp_pos_values = [
        Fix::from_fixed(E),
        alg_fixed!(1.1331485),
        alg_fixed!(1.0157477),
        alg_fixed!(1.0019550),
    ];
    let exp_neg_values = [
        alg_fixed!(0.3678794),
        alg_fixed!(0.8824969),
        alg_fixed!(0.9844964),
        alg_fixed!(0.9980488),
    ];

    if x >= alg_fixed!(10.3972) {
        // The maximum value is often used in the context of adding one so it is the best
        // dealt here (won't have significant impact to the formulas)
        MEAN_VARIANCE_ESTIMATOR_FIX16_MAX - alg_fixed!(1)
    } else if x <= alg_fixed!(-11.7835) {
        ZERO
    } else {
        // I guess we need to calculate (read:approximate this)
        let mut x = x;

        let val = if x < ZERO {
            x = -x;
            exp_neg_values
        } else {
            exp_pos_values
        };

        let mut res = alg_fixed!(1);
        let mut arg = alg_fixed!(1);

        for &v in &val {
            while x >= arg {
                res *= v;
                x -= arg;
            }
            arg >>= 3;
        }
        res
    }
}

struct SigmoidScaledInit {
    offset: Fix,
}

impl SigmoidScaledInit {
    fn new(offset: Fix) -> Self {
        SigmoidScaledInit { offset }
    }

    fn process(&self, sample: Fix) -> Fix {
        let sigmoid_k = Fix::from_num(-0.0065);
        let x = sigmoid_k * (sample - SIGMOID_X0);

        if x < alg_fixed!(-50) {
            SIGMOID_L
        } else if x > alg_fixed!(50) {
            ZERO
        } else {
            //println!("Sample {}, offset:{} X:{}", sample, self.offset, x);
            if sample >= alg_fixed!(0) {
                let shift = (SIGMOID_L - (alg_fixed!(5) * self.offset)) / alg_fixed!(4);
                ((SIGMOID_L + shift) / (alg_fixed!(1) + fixed_exp(x))) - shift
            } else {
                //println!("X^{}", SigmoidScaledInit::exp(x));
                (self.offset / VOC_INDEX_OFFSET_DEFAULT)
                    * (SIGMOID_L / (alg_fixed!(1) + fixed_exp(x)))
            }
        }
    }
}

struct MoxModel {
    sraw_std: Fix,
    sraw_mean: Fix,
}

impl MoxModel {
    fn new(sraw_std: Fix, sraw_mean: Fix) -> Self {
        MoxModel {
            sraw_std,
            sraw_mean,
        }
    }

    fn process(&self, sraw: Fix) -> Fix {
        ((sraw - self.sraw_mean) / (-(self.sraw_std + SRAW_STD_BONUS))) * VOC_INDEX_GAIN
    }
}

#[allow(non_snake_case)]
struct AdaptiveLowpass {
    A1: Fix,
    A2: Fix,
    X1: Fix,
    X2: Fix,
    X3: Fix,
    initialized: bool,
}

impl AdaptiveLowpass {
    fn new() -> Self {
        AdaptiveLowpass {
            A1: SAMPLING_INTERVAL / (LP_TAU_FAST + SAMPLING_INTERVAL),
            A2: SAMPLING_INTERVAL / (LP_TAU_SLOW + SAMPLING_INTERVAL),
            initialized: false,
            X1: ZERO,
            X2: ZERO,
            X3: ZERO,
        }
    }

    fn process(&mut self, sample: Fix) -> Fix {
        if !self.initialized {
            self.X1 = sample;
            self.X2 = sample;
            self.X3 = sample;
            self.initialized = true;
        }

        // TODO: Hopefully, the future version of Fixed crates help me to make this const
        let lp_alpha = Fix::from_num(-0.2);

        self.X1 = (alg_fixed!(1) - self.A1) * self.X1 + self.A1 * sample;
        self.X2 = (alg_fixed!(1) - self.A2) * self.X2 + self.A2 * sample;

        let abs_delta = (self.X1 - self.X2).abs();
        let f1 = fixed_exp(lp_alpha * abs_delta);
        let tau_a = ((LP_TAU_SLOW - LP_TAU_FAST) * f1) + LP_TAU_FAST;
        let a3 = SAMPLING_INTERVAL / (SAMPLING_INTERVAL + tau_a);
        self.X3 = (alg_fixed!(1) - a3) * self.X3 + a3 * sample;
        self.X3
    }
}
