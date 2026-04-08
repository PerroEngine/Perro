    var cloud_mask = 0.0;

    if (ray.y > CLOUD_BASE_HEIGHT) {
        let cloud_size     = clamp(sky.params0.x, 0.0, 1.0);
        let cloud_variance = clamp(sky.params0.z, 0.0, 1.0);
        let wind           = vec2<f32>(sky.wind.x, sky.wind.y);
        let wind_len       = max(length(wind), 1.0e-4);
        let wind_dir       = wind / wind_len;

        let sky_uv_y_adjusted = max(ray.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let sky_uv_raw        = ray.xz / sqrt(sky_uv_y_adjusted);
        let pole_softening    = smoothstep(0.88, 0.995, abs(ray.y));
        let sky_uv            = mix(sky_uv_raw, sky_uv_raw * 0.72, pole_softening);

        let cloud_clock      = cloud_time_seconds * (0.011 + wind_len * 0.06);
        let clouds_scale     = mix(1.55, 0.55, cloud_size);
        let clouds_fuzziness = mix(0.035, 0.24, cloud_variance);
        let coverage_bias    = mix(0.02,  0.14, cloud_density);

        // ── Night wisp turbulence ─────────────────────────────
        // Two sub-wind pocket fields that pull clouds apart at night.
        // Ramps with night_t so daytime clouds are completely unaffected.
        let night_pocket_strength = clamp(night_t * 0.72, 0.0, 1.0);
        let night_turb_uv = sky_uv * (clouds_scale * 0.62)
            + wind_dir * cloud_clock * 1.55
            + vec2<f32>(81.0, -53.0);
        let night_pocket_a = perlin_fbm(night_turb_uv * 1.30
            + vec2<f32>( 17.0,  -9.0), 3) * 2.0 - 1.0;
        let night_pocket_b = perlin_fbm(night_turb_uv * 0.85
            + vec2<f32>(-31.0,  23.0), 3) * 2.0 - 1.0;

        // ── Wind shear ────────────────────────────────────────
        let MAX_SHEAR_RAD = 0.436;
        let dist_t = clamp(length(sky_uv) / 6.0, 0.0, 1.0);
        let shear_field = perlin_fbm(sky_uv * 0.08 + vec2<f32>(53.0, -37.0), 2)
                        * 2.0 - 1.0;
        let shear_top = MAX_SHEAR_RAD * dist_t * shear_field;
        let shear_mid = MAX_SHEAR_RAD * dist_t * shear_field
                      * mix(0.68, 0.82, hash12(vec2<f32>(71.0, 13.0)));
        let shear_bot = MAX_SHEAR_RAD * dist_t * shear_field
                      * mix(0.38, 0.55, hash12(vec2<f32>(29.0, 57.0)));
        let shear_low  = MAX_SHEAR_RAD * dist_t * shear_field * 0.44;
        let shear_high = MAX_SHEAR_RAD * dist_t * shear_field * 0.72;

        // ── Macro warping ─────────────────────────────────────
        let macro_uv = sky_uv * (clouds_scale * 0.45) + vec2<f32>(41.0, -29.0);
        let macro_warp = vec2<f32>(
            perlin_fbm(macro_uv * 0.55 + vec2<f32>( 13.0,  -7.0), 2),
            perlin_fbm(macro_uv * 0.55 + vec2<f32>( -5.0,  11.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let macro_shape = perlin_fbm(macro_uv + macro_warp * 1.35, 3);
        let macro_bias  = smoothstep(0.35, 1.0, macro_shape) * 0.22;

        // ── Top layer ─────────────────────────────────────────
        var drift  = rotate2(wind_dir, shear_top) * cloud_clock * 1.12
                    + vec2<f32>(-0.014, -0.005);
        let top_uv = (sky_uv + drift) * clouds_scale;
        let top_warp = vec2<f32>(
            perlin_fbm(top_uv * 0.7 + vec2<f32>( 19.0, -23.0), 2),
            perlin_fbm(top_uv * 0.7 + vec2<f32>(-17.0,  29.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_top_raw = perlin_fbm(top_uv + top_warp * 0.95, 4);

        // ── Mid layer ─────────────────────────────────────────
        drift = rotate2(wind_dir, shear_mid) * cloud_clock * 0.94
              + vec2<f32>(0.009, 0.012);
        let mid_uv = (sky_uv + drift) * clouds_scale;
        let mid_warp = vec2<f32>(
            perlin_fbm(mid_uv * 0.68 + vec2<f32>(  7.0, -31.0), 2),
            perlin_fbm(mid_uv * 0.68 + vec2<f32>(-29.0,   5.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_middle_raw = perlin_fbm(mid_uv + mid_warp * 0.82, 4);

        // ── Bottom layer ──────────────────────────────────────
        // Uses a tighter scale (1.32x) so the dark shadow underside has a
        // smaller footprint than the top, giving clouds that flare wider
        // at the crown — like real cumulonimbus anvils.
        drift = rotate2(wind_dir, shear_bot) * cloud_clock * 0.83
              + vec2<f32>(-0.006, 0.015);
        let bot_uv = (sky_uv + drift) * (clouds_scale * 1.32);
        let bot_warp = vec2<f32>(
            perlin_fbm(bot_uv * 0.75 + vec2<f32>(-13.0,  17.0), 2),
            perlin_fbm(bot_uv * 0.72 + vec2<f32>( 23.0, -11.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_bottom_raw = perlin_fbm(bot_uv + bot_warp * 0.74, 4);

        // ── Micro detail ──────────────────────────────────────
        let detail_micro = perlin_fbm(
            (sky_uv + wind_dir * cloud_clock * 1.28 + vec2<f32>(9.0, -13.0))
                * (clouds_scale * 1.75),
            3
        );

        // ── Threshold layers ──────────────────────────────────
        let noise_bottom = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_bottom_raw + coverage_bias + macro_bias * 0.80
        ) * 1.10;

        let noise_middle = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_middle_raw + noise_bottom * 0.22
                + coverage_bias * 0.85 + macro_bias * 0.65
        ) * 1.12;

        let noise_top_s = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_top_raw + noise_middle * 0.42
                + coverage_bias * 0.70 + macro_bias * 0.50
        ) * 1.22;

        // ── Extra spread layers ───────────────────────────────
        let low_layer_uv = (sky_uv + rotate2(wind_dir, shear_low) * cloud_clock * 1.25
            + vec2<f32>(-27.0, 14.0)) * (clouds_scale * 0.78);
        let high_layer_uv = (sky_uv + rotate2(wind_dir, shear_high) * cloud_clock * 0.68
            + vec2<f32>(33.0, -21.0)) * (clouds_scale * 1.34);
        let low_layer_raw  = perlin_fbm(low_layer_uv,  3);
        let high_layer_raw = perlin_fbm(high_layer_uv, 3);

        let low_layer = smoothstep(
            clouds_cutoff - 0.06,
            clouds_cutoff + clouds_fuzziness + 0.10,
            low_layer_raw + coverage_bias * 1.05 + macro_bias * 0.55
        );
        let high_layer = smoothstep(
            clouds_cutoff + 0.02,
            clouds_cutoff + clouds_fuzziness + 0.06,
            high_layer_raw + coverage_bias * 0.75 + macro_bias * 0.32
        );

        // ── Base cloud density ────────────────────────────────
        // Bottom layer weight reduced (0.80 vs original 1.15) to match its
        // tighter UV scale — keeps total density balanced.
        var clouds_base = clamp(
            noise_top_s  * 0.80
            + noise_middle * 1.00
            + noise_bottom * 0.80
            + low_layer    * 0.48
            + high_layer   * 0.30,
            0.0, 1.0
        );
        clouds_base = pow(clouds_base, mix(1.0, 0.78, cloud_density));
        clouds_base = clamp(
            clouds_base + cloud_density * 0.14 + macro_bias * 0.18,
            0.0, 1.0
        );

        // ── Erosion / puff / breakup ──────────────────────────
        let realistic_erosion      = smoothstep(0.18, 0.88, noise_top_raw);
        let realistic_puffs        = smoothstep(0.34, 0.90, noise_middle_raw);
        let realistic_fine_breakup = smoothstep(0.22, 0.86, noise_bottom_raw);

        var clouds_amount_real = clouds_base * mix(0.82, 0.96, realistic_puffs);
        clouds_amount_real *= mix(1.02, 0.70, realistic_erosion);
        clouds_amount_real *= mix(1.0,  0.84, realistic_fine_breakup);
        clouds_amount_real  = pow(clamp(clouds_amount_real, 0.0, 1.0), 1.5);

        // ── Wisp masks ────────────────────────────────────────
        let wisp_mask   = smoothstep(0.34, 0.84,
            noise_top_s * (1.0 - noise_bottom));
        let upper_veil  = smoothstep(0.10, 0.72, noise_top_s)
                        * (1.0 - smoothstep(0.56, 1.0, noise_bottom));
        let curved_mass = smoothstep(0.22, 0.86,
            noise_top_s * 0.72 + noise_middle * 0.28);

        let ray_h_cloud   = normalize(vec2<f32>(ray.x, ray.z) + vec2<f32>(1.0e-5));
        let cloud_sun_dot = clamp(dot(ray_h_cloud, sun_h) * 0.5 + 0.5, 0.0, 1.0);
        let sun_behind    = (sun_cloud_cover / 100.0) * day_t;

        let wisp_expanded = smoothstep(0.18, 0.72,
            noise_top_s * (1.0 - noise_bottom * 0.6));
        let wisp_sun = mix(wisp_mask, wisp_expanded,
            sun_behind * cloud_sun_dot * 0.85);

        clouds_amount_real  = mix(clouds_amount_real, curved_mass, 0.26);
        clouds_amount_real += upper_veil * wisp_sun
            * mix(0.24, 0.62, sun_behind * cloud_sun_dot);

        let micro_break = smoothstep(0.32, 0.86, detail_micro);
        clouds_amount_real *= mix(1.08, 0.86, micro_break);
        clouds_amount_real  = pow(clamp(clouds_amount_real, 0.0, 1.0), 0.96);

        let horizon_continuity = smoothstep(
            CLOUD_BASE_HEIGHT, CLOUD_BASE_HEIGHT + 0.06, ray.y);
        clouds_amount_real *= horizon_continuity * 1.1;
        clouds_amount_real *= 1.0 - pole_softening * 0.22;
        clouds_amount_real  = clamp(clouds_amount_real, 0.0, 1.0);

        // ── Night pocket breakup ──────────────────────────────
        // Two overlapping turbulence fields punch holes and fray edges.
        // A separate wisp pass thins the cloud silhouette into fibres.
        // Both are gated by night_pocket_strength so day clouds are pristine.
        let pocket_warp = sky_uv * (clouds_scale * 0.95)
            + vec2<f32>(night_pocket_a, night_pocket_b) * 0.55;
        let pocket_noise = perlin_fbm(
            pocket_warp + vec2<f32>(cloud_clock * 0.8), 3);
        let pocket_holes = smoothstep(0.38, 0.72, pocket_noise);

        let wisp_night = perlin_fbm(
            sky_uv * (clouds_scale * 1.80)
            + wind_dir * cloud_clock * 1.75
            + vec2<f32>(-11.0, 47.0), 4
        );
        let wisp_fray = smoothstep(0.42, 0.88, wisp_night);

        let night_breakup = mix(
            clamp(pocket_holes * 0.60 + wisp_fray * 0.40, 0.0, 1.0),
            0.0,
            1.0 - night_pocket_strength
        );
        clouds_amount_real *= mix(
            1.0,
            clamp(1.0 - night_breakup * 0.78, 0.15, 1.0),
            night_pocket_strength
        );
        // Feather edges more aggressively at night → gossamer wisp look.
        let night_edge_feather = mix(0.0, 0.30, night_pocket_strength)
            * (1.0 - smoothstep(0.18, 0.55, clouds_amount_real));
        clouds_amount_real = clamp(
            clouds_amount_real - night_edge_feather, 0.0, 1.0);

        // ── Base cloud colour ─────────────────────────────────
        let clouds_edge_color_real   = vec3<f32>(0.74, 0.78, 0.86);
        let clouds_top_color_real    = vec3<f32>(0.90, 0.92, 0.95);
        let clouds_middle_color_real = vec3<f32>(0.82, 0.86, 0.92);
        let clouds_bottom_color_real = vec3<f32>(0.70, 0.74, 0.82);

        var clouds_color_real = mix(vec3<f32>(0.0), clouds_top_color_real,    noise_top_s);
        clouds_color_real     = mix(clouds_color_real, clouds_middle_color_real, noise_middle);
        clouds_color_real     = mix(clouds_color_real, clouds_bottom_color_real, noise_bottom);
        clouds_color_real    += vec3<f32>(0.04, 0.05, 0.07) * low_layer;
        clouds_color_real    += vec3<f32>(0.03, 0.04, 0.05) * high_layer;
        clouds_color_real     = mix(clouds_edge_color_real, clouds_color_real, noise_top_s);

        let micro_contrast = smoothstep(0.22, 0.82, detail_micro);
        clouds_color_real *= mix(0.90, 1.04, micro_contrast);

        // ── Self-shadow ───────────────────────────────────────
        let self_shadow = smoothstep(0.20, 0.92, realistic_erosion)
                        * (0.22 + (1.0 - realistic_puffs) * 0.40);
        clouds_color_real *= vec3<f32>(
            1.0 - self_shadow * 0.28,
            1.0 - self_shadow * 0.24,
            1.0 - self_shadow * 0.18
        );

        // ── 3D volume shading ─────────────────────────────────
        let view_up_dot = clamp(ray.y / max(ray.y + 0.12, 1.0e-4), 0.0, 1.0);
        let cloud_layer_height = clamp(
            noise_top_s * 0.55 + noise_middle * 0.30 + noise_bottom * 0.15,
            0.0, 1.0
        );
        let top_face_light = clamp(view_up_dot * cloud_layer_height, 0.0, 1.0);
        let top_roundness  = pow(top_face_light, 1.6);
        let top_col_boost  = mix(vec3<f32>(1.0), vec3<f32>(1.22, 1.18, 1.14), top_roundness);
        let bottom_shadow_view = clamp(
            (1.0 - view_up_dot) * (1.0 - cloud_layer_height), 0.0, 1.0);
        let bottom_shadow_str = pow(bottom_shadow_view, 1.4)
                              * smoothstep(0.30, 0.85, clouds_amount_real);
        let bottom_col_damp   = mix(
            vec3<f32>(1.0), vec3<f32>(0.52, 0.54, 0.60),
            bottom_shadow_str * 0.70
        );
        let sun_top_bonus = clamp(sun_dir.y * 0.6 + 0.4, 0.0, 1.0) * day_t;
        let top_sun_tint  = mix(
            vec3<f32>(1.0), vec3<f32>(1.10, 1.06, 0.96),
            top_roundness * sun_top_bonus * 0.45
        );
        clouds_color_real = clouds_color_real
                          * top_col_boost * bottom_col_damp * top_sun_tint;

        // ── Backlit shadow + silver lining ────────────────────
        let sun_angle_to_ray = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let corona_phase     = pow(sun_angle_to_ray, 12.0);
        let cloud_edge_rim = smoothstep(0.04, 0.38, clouds_amount_real)
                           * (1.0 - smoothstep(0.38, 0.62, clouds_amount_real));
        let corona_intensity = corona_phase * cloud_edge_rim * sun_behind;
        let corona_col_day = vec3<f32>(0.85, 0.72, 0.52);
        let corona_col_eve = vec3<f32>(1.30, 0.72, 0.38);
        let corona_col     = mix(corona_col_day, corona_col_eve, evening_t);
        let corona_flicker  = 0.94 + 0.06 * fbm2(
            top_uv * 1.8 + vec2<f32>(cloud_time_seconds * 0.02, 0.0));
        clouds_color_real += corona_col * corona_intensity * corona_flicker * 0.95;

        let solid_interior = smoothstep(0.42, 0.82, clouds_amount_real);
        let backlit_shadow = sun_behind * corona_phase * solid_interior;
        clouds_color_real *= mix(1.0, 0.38, backlit_shadow * 0.72);

        let thin_cloud   = 1.0 - smoothstep(0.18, 0.55, clouds_amount_real);
        let transmission = corona_phase * sun_behind * thin_cloud;
        let transmit_col = mix(
            vec3<f32>(1.10, 1.02, 0.82),
            vec3<f32>(1.15, 0.72, 0.48),
            evening_t
        );
        clouds_color_real = mix(
            clouds_color_real,
            clouds_color_real * transmit_col,
            transmission * 0.40
        );

        // ── Wispy fibre cool tint ─────────────────────────────
        let fiber     = smoothstep(0.24, 0.82, noise_top_s * (1.0 - noise_middle));
        let wisp_cool = vec3<f32>(0.88, 0.95, 1.05);
        clouds_color_real = mix(
            clouds_color_real,
            clouds_color_real * wisp_cool,
            fiber * upper_veil * 0.20
        );

        // ── Silver lining ─────────────────────────────────────
        let edge_hint = smoothstep(0.18, 0.78, clamp(
            (noise_top_s - noise_middle * 0.45) + (1.0 - noise_bottom) * 0.12,
            0.0, 1.0
        ));
        let silver_lining = pow(
            clamp(1.0 - abs(dot(ray, sun_dir)), 0.0, 1.0), 7.0) * edge_hint;
        clouds_color_real += vec3<f32>(0.94, 0.92, 0.88) * silver_lining * 0.08;

        // ── Sun punch (daytime) ───────────────────────────────
        if (day_t > 0.5) {
            let sun_col_clouds_real = clamp(
                mix(vec3<f32>(1.00, 0.95, 0.84), vec3<f32>(1.00, 0.62, 0.44), evening_t),
                vec3<f32>(0.0), vec3<f32>(1.0)
            );
            let sun_dist_c = distance(ray, sun_dir);
            let sun_punch  = pow(1.0 - clamp(sun_dist_c, 0.0, 1.0), 5.0);
            clouds_color_real = mix(clouds_color_real, sun_col_clouds_real,
                sun_punch * 0.42);
        }

        // ── Sky tint ──────────────────────────────────────────
        let day_src   = gradient3(sky.day_colors,     0.34);
        let eve_src   = gradient3(sky.evening_colors, 0.43);
        let night_src = gradient3(sky.night_colors,   0.36);

        let day_tint   = stylize_cloud_tint(
            day_src   * vec3<f32>(1.10, 1.04, 0.98), 1.15, 0.92, 0.03);
        let eve_tint   = stylize_cloud_tint(
            eve_src   * vec3<f32>(1.20, 0.94, 1.00), 1.25, 0.90, 0.03);
        let night_tint = stylize_cloud_tint(
            night_src * vec3<f32>(0.98, 0.93, 1.32), 1.22, 0.94, 0.02);

        let sky_tint = clamp(
            day_tint * day_t + eve_tint * evening_t + night_tint * night_t,
            vec3<f32>(0.0), vec3<f32>(2.0)
        );
        clouds_color_real = clouds_color_real
            * mix(vec3<f32>(1.0), sky_tint, 0.10);

        // ── Coloured edge band ────────────────────────────────
        let edge_band = clamp(
            (noise_top_s - noise_middle * 0.45) + (1.0 - noise_bottom) * 0.12,
            0.0, 1.0
        );
        let edge_glow      = smoothstep(0.12, 0.84, edge_band);
        let edge_tint_base = clamp(
            mix(day_tint, eve_tint, evening_t) * vec3<f32>(1.10, 1.00, 1.20),
            vec3<f32>(0.0), vec3<f32>(2.0)
        );

        let edge_phase = fract(
            noise_top_raw * 1.25 + noise_middle_raw * 0.95
            + cloud_time_seconds * 0.035
        );
        let rim_yellow = vec3<f32>(1.20, 1.03, 0.68);
        let rim_pink   = vec3<f32>(1.20, 0.74, 1.00);
        let rim_blue   = vec3<f32>(0.72, 0.92, 1.25);
        let rim_warm   = mix(rim_yellow, rim_pink, smoothstep(0.18, 0.56, edge_phase));
        let rim_mix    = mix(rim_warm,   rim_blue,  smoothstep(0.52, 0.90, edge_phase));
        let edge_tint  = clamp(
            mix(edge_tint_base, rim_mix, 0.55),
            vec3<f32>(0.0), vec3<f32>(2.2)
        );

        let micro_edge = smoothstep(0.30, 0.85, detail_micro)
                       * (1.0 - smoothstep(0.85, 1.0, noise_bottom));
        clouds_color_real += edge_tint * edge_glow * (0.24 + micro_edge * 0.12);

        clouds_color_real = min(clouds_color_real, vec3<f32>(1.15, 1.12, 1.10));

        // ── Night fade / storm ────────────────────────────────
        clouds_color_real = mix(clouds_color_real, color,
            clamp(night_t * 0.75, 0.0, 0.92));
        clouds_color_real = mix(clouds_color_real, vec3<f32>(0.0),
            clouds_weight * 0.9);

        // ── Halo ──────────────────────────────────────────────
        let halo = smoothstep(0.18, 0.62, clouds_amount_real)
                 * (1.0 - smoothstep(0.62, 0.96, clouds_amount_real));
        let halo_tint = mix(
            vec3<f32>(1.00, 0.95, 0.86),
            vec3<f32>(1.00, 0.65, 0.58),
            evening_t
        );
        color += halo_tint * halo * 0.07;

        // ── Composite ─────────────────────────────────────────
        color      = mix(color, clouds_color_real, clouds_amount_real);
        cloud_mask = clouds_amount_real;
    }

    // ── Atmospheric scatter: sky glow in gaps (post-cloud pass) ──
    {
        let scatter_angle = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let scatter_wide  = exp(-max(1.0 - scatter_angle, 0.0) * 2.8) * 0.12;
        let scatter_tight = pow(scatter_angle, 14.0) * 0.22; 
        let scatter_total = (scatter_wide + scatter_tight)
                        * sun_cloud_cover * day_t * sun_visibility;
        let scatter_col = mix(
            vec3<f32>(0.95, 0.90, 0.72),
            vec3<f32>(1.00, 0.62, 0.38),
            evening_t
        );
        color += scatter_col * scatter_total;
    }

    // Small per-pixel dither hides gradient banding in twilight/evening.
    let dither = hash12(in.uv * vec2<f32>(1920.0, 1080.0)
        + vec2<f32>(sky_time * 13.0, -sky_time * 7.0)) - 0.5;
    color += vec3<f32>(dither) * (1.2 / 255.0);

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
