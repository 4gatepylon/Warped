//! `frame_trace` records every frame's timing (the CPU's phases, the GPU's start and end, and when
//! the frame reached the display) and writes it as CSV with a summary of the display's cadence,
//! to find stutter.
//!
//! Interface: `FrameTrace`, `CpuFrameRow`, `CompletedFrame`.
//!
//! Example:
//!     let mut trace: FrameTrace = FrameTrace::default();
//!     trace.push(row);
//!     trace.write(&path, &renderer.take_completed_frames())?;

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

/// `CpuFrameRow` is what the CPU did for one frame, in `CACurrentMediaTime`'s clock and in ms.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpuFrameRow {
    pub frame_index: usize,
    pub loop_start_s: f64,
    pub frame_ms: f32,
    pub dt_ms: f32,
    pub sim_ms: f32,
    pub build_ms: f32,
    pub slot_wait_ms: f32,
    pub drawable_wait_ms: f32,
    pub render_ms: f32,
}

/// `CompletedFrame` is what the GPU and the display did with one frame: when its command buffer
/// ran and when it was shown (0 if it was never shown), in `CACurrentMediaTime`'s clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompletedFrame {
    pub frame_index: usize,
    pub gpu_start_s: f64,
    pub gpu_end_s: f64,
    pub presented_s: f64,
}

/// `HITCH_FRAC` marks a display interval as a hitch when it is this much longer than the median.
const HITCH_FRAC: f64 = 1.5;
/// `MS_PER_S` converts seconds to milliseconds.
const MS_PER_S: f64 = 1000.0;

/// `FrameTrace` holds the CPU side of every frame of a run.
#[derive(Default)]
pub struct FrameTrace {
    rows: Vec<CpuFrameRow>,
}

impl FrameTrace {
    /// `push` records one frame's CPU side.
    ///
    /// Example: `trace.push(CpuFrameRow { frame_index, ..CpuFrameRow::default() });`
    pub fn push(&mut self, row: CpuFrameRow) {
        self.rows.push(row);
    }

    /// `write` joins the CPU rows with `completed` by frame index, writes one CSV row per frame
    /// to `path`, and returns a one-line summary of the display intervals and of how far each
    /// frame's simulated step was from the interval it was shown for.
    ///
    /// Example: `println!("{}", trace.write(&path, &completed)?);`
    pub fn write(&self, path: &Path, completed: &[CompletedFrame]) -> std::io::Result<String> {
        let by_index: HashMap<usize, CompletedFrame> = completed
            .iter()
            .map(|frame| (frame.frame_index, *frame))
            .collect();
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        writeln!(
            file,
            "frame,loop_start_s,frame_ms,dt_ms,sim_ms,build_ms,slot_wait_ms,drawable_wait_ms,render_ms,gpu_ms,gpu_end_s,presented_s,present_interval_ms"
        )?;
        let mut previous_presented_s: Option<f64> = None;
        let mut intervals_ms: Vec<f64> = Vec::new();
        let mut step_errors_ms: Vec<f64> = Vec::new();
        for row in &self.rows {
            let gpu: CompletedFrame = by_index.get(&row.frame_index).copied().unwrap_or_default();
            let interval_ms: f64 = match (previous_presented_s, gpu.presented_s > 0.0) {
                (Some(previous_s), true) => (gpu.presented_s - previous_s) * MS_PER_S,
                _ => 0.0,
            };
            if interval_ms > 0.0 {
                intervals_ms.push(interval_ms);
                step_errors_ms.push((f64::from(row.dt_ms) - interval_ms).abs());
            }
            if gpu.presented_s > 0.0 {
                previous_presented_s = Some(gpu.presented_s);
            }
            writeln!(
                file,
                "{},{:.6},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.6},{:.6},{:.3}",
                row.frame_index,
                row.loop_start_s,
                row.frame_ms,
                row.dt_ms,
                row.sim_ms,
                row.build_ms,
                row.slot_wait_ms,
                row.drawable_wait_ms,
                row.render_ms,
                (gpu.gpu_end_s - gpu.gpu_start_s) * MS_PER_S,
                gpu.gpu_end_s,
                gpu.presented_s,
                interval_ms
            )?;
        }
        file.flush()?;
        Ok(summarize_intervals(&intervals_ms, &step_errors_ms))
    }
}

/// `summarize_intervals` describes the display intervals (median, p99, how many are hitches) and
/// the mean and p99 gap between each frame's simulated step and its display interval.
fn summarize_intervals(intervals_ms: &[f64], step_errors_ms: &[f64]) -> String {
    if intervals_ms.is_empty() {
        return "no frame reached the display".to_string();
    }
    let percentile = |values: &[f64], frac: f64| -> f64 {
        let mut sorted: Vec<f64> = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        sorted[((sorted.len() - 1) as f64 * frac).round() as usize]
    };
    let median_ms: f64 = percentile(intervals_ms, 0.5);
    let hitch_count: usize = intervals_ms
        .iter()
        .filter(|&&ms| ms > median_ms * HITCH_FRAC)
        .count();
    let mean_error_ms: f64 = step_errors_ms.iter().sum::<f64>() / step_errors_ms.len() as f64;
    format!(
        "{} shown frames: interval median {:.2} ms, p90 {:.2} ms, p99 {:.2} ms, worst {:.2} ms, {} hitches (> {HITCH_FRAC} x median); step vs shown interval: mean gap {:.2} ms, p99 gap {:.2} ms",
        intervals_ms.len(),
        median_ms,
        percentile(intervals_ms, 0.9),
        percentile(intervals_ms, 0.99),
        percentile(intervals_ms, 1.0),
        hitch_count,
        mean_error_ms,
        percentile(step_errors_ms, 0.99)
    )
}
