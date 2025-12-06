//! Calibration and offset management for Joy-Con sensors
//!
//! This module provides functionality to calculate and apply offsets to gyroscope
//! and accelerometer readings to account for baseline drift and noise.

use crate::input_report::{Gyro, Accel, Stick};

/// Calibration offsets for gyroscope, accelerometer, and analog stick
#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    /// Gyroscope offsets (degrees per second)
    pub gyro: Gyro,
    /// Accelerometer offsets (g-force)
    pub accel: Accel,
    /// Stick center offsets (raw values, typically around 2048)
    pub stick_center: Stick,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            gyro: Gyro { x: 0.0, y: 0.0, z: 0.0 },
            accel: Accel { x: 0.0, y: 0.0, z: 0.0 },
            stick_center: Stick {
                horizontal: 2048,
                vertical: 2048,
                horizontal_norm: 0.5,
                vertical_norm: 0.5,
                horizontal_percent: 50,
                vertical_percent: 50,
            },
        }
    }
}

impl Calibration {
    /// Create a new calibration with zero offsets
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate offsets from a collection of samples
    /// This averages all samples to find the baseline offset
    pub fn calculate_offset(samples: &[CalibrationSample]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let count = samples.len() as f32;
        
        // Average all samples
        let gyro_x_avg = samples.iter().map(|s| s.gyro.x).sum::<f32>() / count;
        let gyro_y_avg = samples.iter().map(|s| s.gyro.y).sum::<f32>() / count;
        let gyro_z_avg = samples.iter().map(|s| s.gyro.z).sum::<f32>() / count;
        
        let accel_x_avg = samples.iter().map(|s| s.accel.x).sum::<f32>() / count;
        let accel_y_avg = samples.iter().map(|s| s.accel.y).sum::<f32>() / count;
        let accel_z_avg = samples.iter().map(|s| s.accel.z).sum::<f32>() / count;
        
        let stick_h_avg = samples.iter().map(|s| s.stick.horizontal as f32).sum::<f32>() / count;
        let stick_v_avg = samples.iter().map(|s| s.stick.vertical as f32).sum::<f32>() / count;

        Self {
            gyro: Gyro {
                x: gyro_x_avg,
                y: gyro_y_avg,
                z: gyro_z_avg,
            },
            accel: Accel {
                x: accel_x_avg,
                y: accel_y_avg,
                z: accel_z_avg,
            },
            stick_center: Stick {
                horizontal: stick_h_avg as u16,
                vertical: stick_v_avg as u16,
                horizontal_norm: stick_h_avg / 4095.0,
                vertical_norm: stick_v_avg / 4095.0,
                horizontal_percent: ((stick_h_avg / 4095.0) * 100.0) as u8,
                vertical_percent: ((stick_v_avg / 4095.0) * 100.0) as u8,
            },
        }
    }

    /// Apply calibration offsets to raw gyroscope data
    /// Returns calibrated values (raw - offset)
    pub fn apply_gyro(&self, raw: Gyro) -> Gyro {
        Gyro {
            x: raw.x - self.gyro.x,
            y: raw.y - self.gyro.y,
            z: raw.z - self.gyro.z,
        }
    }

    /// Apply calibration offsets to raw accelerometer data
    /// Returns raw values unchanged (accelerometer should show absolute G-forces, not relative to calibration position)
    /// Unlike gyroscope, accelerometer values need to show absolute forces (e.g., 4000 = 1G, 8000 = 2G)
    /// so we don't subtract the calibration offset
    pub fn apply_accel(&self, raw: Accel) -> Accel {
        // Return raw values - don't subtract offset
        // Accelerometer should show absolute G-forces, not relative to calibration position
        raw
    }

    /// Apply calibration offsets to both gyro and accel
    pub fn apply(&self, raw_gyro: Gyro, raw_accel: Accel) -> (Gyro, Accel) {
        (self.apply_gyro(raw_gyro), self.apply_accel(raw_accel))
    }

    /// Apply calibration offsets to raw stick data
    /// Subtracts center values so rest position becomes 0,0
    /// Then normalizes to -1.0 to 1.0 range (where 0,0 is center)
    /// Applies deadzone so small values near 0 are treated as 0
    pub fn apply_stick(&self, raw: Stick) -> Stick {
        // Subtract center values: if center is 2046, subtract 2046 so rest becomes 0
        // This is the same as gyro/accel calibration (raw - offset)
        let h_offset = raw.horizontal as i32 - self.stick_center.horizontal as i32;
        let v_offset = raw.vertical as i32 - self.stick_center.vertical as i32;
        
        // Estimate the physical range of the stick (typically ±1000-1500 from center)
        // This is an estimate - actual range may vary per controller
        const ESTIMATED_RANGE: f32 = 1500.0; // Maximum offset from center we expect
        
        // Normalize to -1.0 to 1.0 range (0,0 is center)
        let mut h_norm = (h_offset as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
        let mut v_norm = (v_offset as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
        
        // Apply deadzone: small values near 0 are treated as 0
        // Values like -0.0100 and -0.0227 should be considered 0
        const DEADZONE: f32 = 0.03; // Deadzone threshold (3% of range)
        if h_norm.abs() < DEADZONE {
            h_norm = 0.0;
        }
        if v_norm.abs() < DEADZONE {
            v_norm = 0.0;
        }
        
        // Store the offset from center (0 when at rest)
        // When offset = 0, store 0
        let h_calibrated = if h_offset == 0 { 0 } else { h_offset.abs() as u16 };
        let v_calibrated = if v_offset == 0 { 0 } else { v_offset.abs() as u16 };
        
        Stick {
            horizontal: h_calibrated,
            vertical: v_calibrated,
            horizontal_norm: h_norm,
            vertical_norm: v_norm,
            horizontal_percent: 0, // Deprecated, not used
            vertical_percent: 0, // Deprecated, not used
        }
    }
}

/// A sample containing gyro, accel, and stick readings for calibration
#[derive(Debug, Clone, Copy)]
pub struct CalibrationSample {
    pub gyro: Gyro,
    pub accel: Accel,
    pub stick: Stick,
}

impl CalibrationSample {
    /// Create a new sample from gyro, accel, and stick readings
    pub fn new(gyro: Gyro, accel: Accel, stick: Stick) -> Self {
        Self { gyro, accel, stick }
    }
}

/// A sample containing both gyro and accel readings (for backward compatibility)
#[deprecated(note = "Use CalibrationSample instead")]
pub type GyroAccelSample = CalibrationSample;

