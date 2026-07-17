use super::*;

// Hits this close to the listener are the listener's own body (camera inside a
// character collider casts solid rays with t = 0); ignore them.
const LISTENER_EMBED_EPSILON: f32 = 0.05;

// Occluded sounds probe a small cloud of points around the source; the open
// fraction lets energy diffract around edges instead of a binary wall cutoff.
// Two rings: probes near the direct line weigh more than the wide ring, so
// openness rises gradually as the listener sweeps past a corner instead of
// snapping between coarse fractions.
const AUDIO_DIFFUSION_SPREAD: f32 = 1.25;
const AUDIO_DIFFUSION_SPREAD_NEAR: f32 = 0.45;
const AUDIO_DIFFUSION_NEAR_WEIGHT: f32 = 1.0;
const AUDIO_DIFFUSION_FAR_WEIGHT: f32 = 0.6;
// How much of the unoccluded level leaks through a fully open side.
const AUDIO_DIFFUSION_LEAK: f32 = 0.6;

// --- Bidirectional ray reconciliation (Phase 1) ---
// A listener-side and source-side path point reconcile into one aperture when a
// verification raycast between them is unobstructed. Pairs within
// RECONCILE_EPSILON are treated as coincident (tightest matches, preferred in
// scoring); pairs out to RECONCILE_VERIFY_MAX still reconcile if the connecting
// segment is clear. Both cases verify the segment so points straddling a thin
// wall never falsely reconcile.
const RECONCILE_EPSILON: f32 = 0.5;
const RECONCILE_VERIFY_MAX: f32 = 2.0;
// Spacing between free-segment sample points (world units). Fixed spacing keeps
// sample density high near apertures regardless of how far a ray travels, so
// listener-side and source-side points can reconcile within RECONCILE_VERIFY_MAX.
const RECONCILE_SAMPLE_SPACING: f32 = 0.75;
// Cap on samples per segment to bound work on very long free rays.
const RECONCILE_MAX_SAMPLES: usize = 32;
// Full fan re-search cadence: verified caches survive this many ticks before a
// forced re-search even when verification keeps passing.
const APERTURE_RESEARCH_TICKS: u32 = 10;
// Number of ray directions per side in the reconciling fan (2D).
const RECONCILE_FAN_2D: usize = AUDIO_BOUNCE_RAYS_2D;
const RECONCILE_FAN_3D: usize = AUDIO_BOUNCE_RAYS_3D;

// Persistent field (Phase 2): probes refreshed per tick, round-robin. The rest
// keep their stored value, so total openness rays/tick drop from the full fan
// (4/6) to PROBE_SLICE while the blended openness stays stable.
const PROBE_SLICE: usize = 2;
// Openness hysteresis: opening reads fast, closing fades slow (mirror
// smooth_volume in scene.rs) so a probe flipping on alternate ticks does not
// oscillate the level.
const OPENNESS_RISE: f32 = 0.6;
const OPENNESS_FALL: f32 = 0.25;

fn attached_node_of(sound: &ActiveSpatialSound) -> Option<NodeID> {
    match sound.pos {
        SpatialSoundPos::Attached(node) => Some(node),
        _ => None,
    }
}

// Build the shared audio raycast filter once so callers casting many rays
// (probe clouds, reconcile pairs) do not re-alloc the exclude list per ray.
fn audio_raycast_filter(audio_layer: BitMask, attached_node: Option<NodeID>) -> PhysicsQueryFilter {
    PhysicsQueryFilter {
        layers: audio_layer,
        include_areas: false,
        exclude_nodes: attached_node.into_iter().collect(),
        ..PhysicsQueryFilter::default()
    }
}

const AUDIO_DEBUG_DIRECT: [f32; 4] = [0.1, 1.0, 0.55, 1.0];
const AUDIO_DEBUG_THROUGH: [f32; 4] = [0.68, 0.18, 1.0, 0.9];
const AUDIO_DEBUG_BOUNCE: [f32; 4] = [1.0, 0.56, 0.12, 0.9];
const AUDIO_DEBUG_ABSORB: [f32; 4] = [0.34, 0.18, 0.74, 0.55];

#[inline]
fn audio_debug_color(color: [f32; 4], energy: f32) -> [f32; 4] {
    let strength = (0.25 + energy.clamp(0.0, 1.0).sqrt() * 0.75).clamp(0.0, 1.0);
    [
        color[0] * strength,
        color[1] * strength,
        color[2] * strength,
        color[3],
    ]
}

mod bounce;
mod material;
mod occlusion;
mod portals;
mod spatial;

#[derive(Clone, Copy)]
enum EmitterMode {
    Directional,
    InverseDirectional,
    Bidirectional,
}

fn emitter_lobe(mode: EmitterMode, dot: f32) -> f32 {
    match mode {
        EmitterMode::Directional => directional_lobe(dot),
        EmitterMode::InverseDirectional => directional_lobe(-dot),
        EmitterMode::Bidirectional => 0.15 + 0.85 * dot.abs().powf(1.5),
    }
}

fn directional_lobe(dot: f32) -> f32 {
    0.15 + 0.85 * dot.max(0.0).powf(1.5)
}

// Hysteresis blend for stored openness: rise fast on opening, fall slow on
// closing, so a probe flipping blocked/unblocked on alternate ticks does not
// oscillate the perceived level.
#[inline]
fn smooth_openness(prev: f32, next: f32) -> f32 {
    let rate = if next > prev {
        OPENNESS_RISE
    } else {
        OPENNESS_FALL
    };
    prev + (next - prev) * rate
}

fn listener_effect_mix(
    options: perro_structs::AudioListenerOptions,
    audio_layer: BitMask,
) -> AudioEffectZoneMix {
    if options.audio_mask.intersects(audio_layer) {
        return AudioEffectZoneMix::default();
    }
    let mut mix = AudioEffectZoneMix::default();
    for effect in options.effects {
        mix.apply(effect);
    }
    mix
}
