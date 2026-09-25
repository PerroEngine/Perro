//! `PERRO_SPIKE_LOG=<ms>`: one compact line per frame whose active work
//! (update + present, minus present/vsync wait) reaches the threshold.
//! Batch averages hide single-frame hitches; this names the phase that spiked.
//!
//! Output: stdout, plus an append to `PERRO_SPIKE_LOG_FILE` when set.
//! Off (env unset): the runner holds `None` and nothing here runs.

use super::*;
use perro_graphics::spike_counters::GfxSpikeCounters;
use perro_runtime::spike_counters::RuntimeSpikeCounters;

pub(super) struct SpikeLog {
    threshold: Duration,
    file: Option<fs::File>,
    logged: u64,
}

/// Everything the frame loop measured for one frame.
pub(super) struct SpikeFrame<'a> {
    pub frame_index: u64,
    pub frame_delta: Duration,
    pub idle: Duration,
    pub active_work: Duration,
    pub present_wait: Duration,
    pub simulation: Duration,
    pub fixed: Duration,
    pub fixed_steps: u32,
    pub fixed_scripts: Duration,
    pub fixed_physics: Duration,
    pub update: &'a perro_runtime::RuntimeUpdateTiming,
    pub top_script_name: Option<&'a str>,
    pub present_active: Duration,
    pub present: Option<&'a crate::PresentDetailTiming>,
    pub draw: &'a DrawFrameTiming,
}

#[inline]
fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[inline]
fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

impl SpikeLog {
    pub(super) fn from_env() -> Option<Self> {
        let threshold_ms = perro_graphics::spike_counters::spike_log_threshold_ms()?;
        let file = std::env::var_os("PERRO_SPIKE_LOG_FILE").and_then(|path| {
            let path = std::path::PathBuf::from(path);
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                let _ = fs::create_dir_all(parent);
            }
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });
        eprintln!(
            "[perro][spike] logging frames with active work >= {threshold_ms}ms{}",
            if file.is_some() {
                " (also appending to PERRO_SPIKE_LOG_FILE)"
            } else {
                ""
            }
        );
        Some(Self {
            threshold: Duration::from_secs_f64(threshold_ms / 1000.0),
            file,
            logged: 0,
        })
    }

    /// Drain this frame's counters (always, so they stay per-frame) and log
    /// when the frame crossed the threshold.
    pub(super) fn observe(&mut self, frame: &SpikeFrame<'_>) {
        let gfx = perro_graphics::spike_counters::take();
        let rt = perro_runtime::spike_counters::take();
        if frame.active_work < self.threshold {
            return;
        }
        self.logged = self.logged.saturating_add(1);
        let line = Self::format_line(frame, &gfx, &rt);
        {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{line}");
        }
        if let Some(file) = &mut self.file {
            let _ = writeln!(file, "{line}");
        }
    }

    pub(super) fn is_triggered(&self, active_work: Duration) -> bool {
        active_work >= self.threshold
    }

    fn format_line(
        f: &SpikeFrame<'_>,
        gfx: &GfxSpikeCounters,
        rt: &RuntimeSpikeCounters,
    ) -> String {
        use std::fmt::Write as _;
        let d = f.draw;
        let u = f.update;
        let mut s = String::with_capacity(768);
        let _ = write!(
            s,
            "[perro][spike] f#{} work={:.2}ms delta={:.2} idle={:.2} present_wait={:.2}",
            f.frame_index,
            ms(f.active_work),
            ms(f.frame_delta),
            ms(f.idle),
            ms(f.present_wait),
        );
        // simulation
        let _ = write!(
            s,
            " | sim={:.2} fixed={:.2}(steps={} scr={:.2} phys={:.2}) start={:.2} upd_scr={:.2}(n={})",
            ms(f.simulation),
            ms(f.fixed),
            f.fixed_steps,
            ms(f.fixed_scripts),
            ms(f.fixed_physics),
            ms(u.start_schedule),
            ms(u.update_schedule.scripts_total),
            u.update_schedule.script_count,
        );
        if let Some(id) = u.update_schedule.slowest_script_id {
            let _ = write!(
                s,
                " top={}\"{}\"={:.2}",
                id.as_u64(),
                f.top_script_name.unwrap_or("?"),
                ms(u.update_schedule.slowest_script),
            );
        }
        let _ = write!(s, " internal={:.2}", ms(u.internal_update));
        // present / extraction
        let _ = write!(s, " | present={:.2}", ms(f.present_active));
        if let Some(p) = f.present {
            let _ = write!(
                s,
                " x2d={:.2} x3d={:.2} xui={:.2}(layout={:.2} cmds={:.2} dirty={}) drain+submit={:.2}(cmds={}) events={:.2}",
                ms(p.extract_2d),
                ms(p.extract_3d),
                ms(p.extract_ui),
                ms(p.ui_layout),
                ms(p.ui_commands),
                p.ui_dirty_nodes,
                ms(p.drain_submit),
                p.render_commands,
                ms(p.apply_events),
            );
        }
        // draw
        let _ = write!(
            s,
            " | draw={:.2} proc={:.2} prep={:.2} p2d={:.2} p3d={:.2} acq={:.2} enc={:.2} stream_enc={:.2} submit={:.2} post={:.2} pres={:.2}",
            ms(d.total),
            ms(d.process_commands),
            ms(d.prepare_cpu),
            ms(d.gpu_prepare_2d),
            ms(d.gpu_prepare_3d),
            ms(d.gpu_acquire),
            ms(d.gpu_encode_main),
            ms(d.gpu_stream_encode),
            ms(d.gpu_submit_main),
            ms(d.gpu_post_process),
            ms(d.gpu_present),
        );
        if !d.gpu_timestamp_main.is_zero() {
            let _ = write!(
                s,
                " gpu_ts(lagged)={:.2}(mesh={:.2} shadow={:.2} post={:.2})",
                ms(d.gpu_timestamp_main),
                ms(d.gpu_timestamp_mesh),
                ms(d.gpu_timestamp_shadow),
                ms(d.gpu_timestamp_post),
            );
        }
        // camera streams / sub-views
        let _ = write!(
            s,
            " | streams n={} rendered={} px={} upsert={} chg={} new={} resume={} suspend={} rm={} tex_resize={} rt_alloc={}/{:.2}MiB",
            d.stream_count,
            d.stream_renders,
            d.stream_pixels,
            gfx.stream_upserts,
            gfx.stream_changed,
            gfx.stream_new,
            gfx.stream_resumes,
            gfx.stream_suspends,
            gfx.stream_removes,
            gfx.stream_tex_resizes,
            gfx.stream_rt_allocs,
            mib(gfx.stream_rt_bytes),
        );
        // nodes / scenes / uploads
        let _ = write!(
            s,
            " | nodes +{} -{} scenes={}/{:.2}ms | upload mesh={}/{:.2}MiB tex={}/{:.2}MiB",
            rt.nodes_created,
            rt.nodes_removed,
            rt.scene_loads,
            rt.scene_load_ns as f64 / 1_000_000.0,
            gfx.mesh_uploads,
            mib(gfx.mesh_upload_bytes),
            gfx.tex_uploads,
            mib(gfx.tex_upload_bytes),
        );
        // Pipeline creation calls. These are exact per-frame counts from the
        // engine funnel and do not need an expensive driver counter query.
        let _ = write!(
            s,
            " | pipes={}r/{}c 3d_sets={} warm_pending={}",
            gfx.render_pipeline_builds,
            gfx.compute_pipeline_builds,
            d.pipeline_compiles_3d,
            d.pipeline_warms_pending_3d,
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spike_line_names_every_section() {
        let update = perro_runtime::RuntimeUpdateTiming::default();
        let draw = DrawFrameTiming {
            stream_count: 10,
            stream_renders: 10,
            pipeline_compiles_3d: 2,
            ..Default::default()
        };
        let present = crate::PresentDetailTiming::default();
        let frame = SpikeFrame {
            frame_index: 42,
            frame_delta: Duration::from_millis(40),
            idle: Duration::ZERO,
            active_work: Duration::from_millis(38),
            present_wait: Duration::from_millis(1),
            simulation: Duration::from_millis(3),
            fixed: Duration::from_millis(1),
            fixed_steps: 1,
            fixed_scripts: Duration::ZERO,
            fixed_physics: Duration::ZERO,
            update: &update,
            top_script_name: None,
            present_active: Duration::from_millis(30),
            present: Some(&present),
            draw: &draw,
        };
        let gfx = GfxSpikeCounters {
            stream_new: 10,
            stream_rt_allocs: 10,
            stream_rt_bytes: 10 * 256 * 256 * 4,
            render_pipeline_builds: 4,
            ..Default::default()
        };
        let line = SpikeLog::format_line(&frame, &gfx, &RuntimeSpikeCounters::default());
        assert!(
            line.starts_with("[perro][spike] f#42 work=38.00ms"),
            "{line}"
        );
        for needle in [
            " sim=",
            " xui=",
            " draw=",
            " streams n=10 rendered=10",
            " new=10",
            " rt_alloc=10/2.50MiB",
            " nodes +0 -0",
            " pipes=4r/0c",
            " 3d_sets=2",
        ] {
            assert!(line.contains(needle), "missing `{needle}` in {line}");
        }
        assert!(!line.contains('\n'));
    }
}
