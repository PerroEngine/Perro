//! Calibration and offset management for Joy-Con sensors
//!
//! This module provides functionality to calculate and apply offsets to gyroscope
//! and accelerometer readings to account for baseline drift and noise.

use crate::structs::{Vector2, Vector3};

/// Calibration offsets for gyroscope, accelerometer, and analog stick
#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    /// Gyroscope offsets (degrees per second)
    pub gyro: Vector3,
    /// Accelerometer offsets (g-force)
    pub accel: Vector3,
    /// Stick center offset (normalized, typically 0.0, 0.0)
    pub stick_center: Vector2,
    /// Gyroscope deadzone threshold (degrees per second)
    /// Values below this threshold are treated as zero to filter noise
    pub gyro_deadzone: f32,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            gyro: Vector3::ZERO,
            accel: Vector3::ZERO,
            stick_center: Vector2::ZERO,
            // Default deadzone: 5.0 degrees/second (approximately 0.087 radians/second)
            // This filters out small noise while still allowing intentional movements
            gyro_deadzone: 5.0,
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
    /// Also calculates noise level to determine appropriate deadzone
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

        let stick_x_avg = samples.iter().map(|s| s.stick.x).sum::<f32>() / count;
        let stick_y_avg = samples.iter().map(|s| s.stick.y).sum::<f32>() / count;

        // Calculate noise level: standard deviation of gyro values
        // This helps determine an appropriate deadzone threshold
        let gyro_x_variance = samples
            .iter()
            .map(|s| (s.gyro.x - gyro_x_avg).powi(2))
            .sum::<f32>()
            / count;
        let gyro_y_variance = samples
            .iter()
            .map(|s| (s.gyro.y - gyro_y_avg).powi(2))
            .sum::<f32>()
            / count;
        let gyro_z_variance = samples
            .iter()
            .map(|s| (s.gyro.z - gyro_z_avg).powi(2))
            .sum::<f32>()
            / count;

        let gyro_x_stddev = gyro_x_variance.sqrt();
        let gyro_y_stddev = gyro_y_variance.sqrt();
        let gyro_z_stddev = gyro_z_variance.sqrt();

        // Use 3x the average standard deviation as deadzone threshold
        // This filters out ~99.7% of noise (3-sigma rule) while preserving intentional movements
        let avg_stddev = (gyro_x_stddev + gyro_y_stddev + gyro_z_stddev) / 3.0;
        let gyro_deadzone = (avg_stddev * 3.0).max(5.0); // Minimum 5.0 deg/s, or 3x noise level

        Self {
            gyro: Vector3::new(gyro_x_avg, gyro_y_avg, gyro_z_avg),
            accel: Vector3::new(accel_x_avg, accel_y_avg, accel_z_avg),
            stick_center: Vector2::new(stick_x_avg, stick_y_avg),
            gyro_deadzone,
        }
    }

    /// Apply calibration offsets to raw gyroscope data
    /// Returns calibrated values (raw - offset)
    /// Also applies deadzone and axis biasing:
    /// - When one axis is dominant, other axes get a larger deadzone to prevent unwanted rotation
    /// - Base deadzone filters out noise on all axes
    pub fn apply_gyro(&self, raw: Vector3) -> Vector3 {
        // First, subtract calibration offset
        let mut calibrated = Vector3::new(
            raw.x - self.gyro.x,
            raw.y - self.gyro.y,
            raw.z - self.gyro.z,
        );

        // Find the dominant axis (largest absolute value)
        let abs_x = calibrated.x.abs();
        let abs_y = calibrated.y.abs();
        let abs_z = calibrated.z.abs();

        // Threshold for considering an axis "dominant" (2x larger than others)
        // This prevents unwanted rotation on secondary axes when primarily rotating on one axis
        const DOMINANCE_THRESHOLD: f32 = 2.0;

        // Determine which axis is dominant (if any)
        let dominant_axis = if abs_x >= abs_y
            && abs_x >= abs_z
            && abs_x > abs_y.max(abs_z) * DOMINANCE_THRESHOLD
        {
            Some(0) // X is dominant
        } else if abs_y >= abs_x && abs_y >= abs_z && abs_y > abs_x.max(abs_z) * DOMINANCE_THRESHOLD
        {
            Some(1) // Y is dominant
        } else if abs_z >= abs_x && abs_z >= abs_y && abs_z > abs_x.max(abs_y) * DOMINANCE_THRESHOLD
        {
            Some(2) // Z is dominant
        } else {
            None // No single axis is dominant
        };

        // Apply deadzone with axis biasing
        // When one axis is dominant, other axes get a larger deadzone (2x base deadzone)
        // This prevents unwanted rotation on secondary axes during intentional single-axis rotation
        let base_deadzone = self.gyro_deadzone;
        let secondary_deadzone = base_deadzone * 2.0; // Larger deadzone for non-dominant axes

        // Apply deadzone to each axis
        if let Some(dominant) = dominant_axis {
            // One axis is dominant - apply larger deadzone to other axes
            if dominant != 0 && abs_x < secondary_deadzone {
                calibrated.x = 0.0;
            } else if dominant == 0 && abs_x < base_deadzone {
                calibrated.x = 0.0;
            }

            if dominant != 1 && abs_y < secondary_deadzone {
                calibrated.y = 0.0;
            } else if dominant == 1 && abs_y < base_deadzone {
                calibrated.y = 0.0;
            }

            if dominant != 2 && abs_z < secondary_deadzone {
                calibrated.z = 0.0;
            } else if dominant == 2 && abs_z < base_deadzone {
                calibrated.z = 0.0;
            }
        } else {
            // No dominant axis - apply base deadzone to all axes
            if abs_x < base_deadzone {
                calibrated.x = 0.0;
            }
            if abs_y < base_deadzone {
                calibrated.y = 0.0;
            }
            if abs_z < base_deadzone {
                calibrated.z = 0.0;
            }
        }

        calibrated
    }

    /// Apply calibration offsets to raw accelerometer data
    /// Returns raw values unchanged (accelerometer should show absolute G-forces, not relative to calibration position)
    /// Unlike gyroscope, accelerometer values need to show absolute forces (e.g., 4000 = 1G, 8000 = 2G)
    /// so we don't subtract the calibration offset
    pub fn apply_accel(&self, raw: Vector3) -> Vector3 {
        // Return raw values - don't subtract offset
        // Accelerometer should show absolute G-forces, not relative to calibration position
        raw
    }

    /// Apply calibration offsets to both gyro and accel
    pub fn apply(&self, raw_gyro: Vector3, raw_accel: Vector3) -> (Vector3, Vector3) {
        (self.apply_gyro(raw_gyro), self.apply_accel(raw_accel))
    }

    /// Apply calibration offsets to stick data
    /// Subtracts center offset so rest position becomes 0,0
    /// Applies deadzone so small values near 0 are treated as 0
    pub fn apply_stick(&self, raw: Vector2) -> Vector2 {
        // Subtract center offset (stick is already normalized to -1.0 to 1.0)
        let mut x = raw.x - self.stick_center.x;
        let mut y = raw.y - self.stick_center.y;

        // Apply deadzone: small values near 0 are treated as 0
        const DEADZONE: f32 = 0.03; // Deadzone threshold (3% of range)
        if x.abs() < DEADZONE {
            x = 0.0;
        }
        if y.abs() < DEADZONE {
            y = 0.0;
        }

        Vector2::new(x, y)
    }
}

/// A sample containing gyro, accel, and stick readings for calibration
#[derive(Debug, Clone, Copy)]
pub struct CalibrationSample {
    pub gyro: Vector3,
    pub accel: Vector3,
    pub stick: Vector2,
}

impl CalibrationSample {
    /// Create a new sample from gyro, accel, and stick readings
    pub fn new(gyro: Vector3, accel: Vector3, stick: Vector2) -> Self {
        Self { gyro, accel, stick }
    }
}

/// A sample containing both gyro and accel readings (for backward compatibility)
#[deprecated(note = "Use CalibrationSample instead")]
pub type GyroAccelSample = CalibrationSample;
