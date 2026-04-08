    // ── Cloud cover sample in sun direction ───────────────────
    let cloud_time_seconds = sky.params2.w;
    var sun_cloud_cover    = 0.0;
    if (sun_dir.y > CLOUD_BASE_HEIGHT) {
        let sun_uv_y       = max(sun_dir.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let sun_sky_uv     = sun_dir.xz / sqrt(sun_uv_y);
        let cloud_clock_sun = sky_time
            * (0.011 + length(vec2<f32>(sky.wind.x, sky.wind.y)) * 0.06);
        let wind_s         = vec2<f32>(sky.wind.x, sky.wind.y);
        let wind_dir_sun   = wind_s / max(length(wind_s), 1.0e-4);
        let cloud_size_s   = clamp(sky.params0.x, 0.0, 1.0);
        let clouds_scale_s = mix(1.55, 0.55, cloud_size_s);

        let drift_sun    = wind_dir_sun * cloud_clock_sun * 1.12
                         + vec2<f32>(-0.014, -0.005);
        let sun_cloud_uv = (sun_sky_uv + drift_sun) * clouds_scale_s;
        let sun_cloud_raw     = perlin_fbm(sun_cloud_uv, 2);
        let cloud_density_s   = clamp(sky.params0.y, 0.0, 1.0);
        let clouds_cutoff_s   = mix(0.8, 0.52, cloud_density_s);
        let cloud_variance_s  = clamp(sky.params0.z, 0.0, 1.0);
        let clouds_fuzziness_s = mix(0.035, 0.28, cloud_variance_s);
        sun_cloud_cover = smoothstep(
            clouds_cutoff_s - 0.08,
            clouds_cutoff_s + clouds_fuzziness_s + 0.12,
            sun_cloud_raw + cloud_density_s * 0.10
        );
        let sun_cover_fade = smoothstep(
            CLOUD_BASE_HEIGHT,
            CLOUD_BASE_HEIGHT + 0.2,
            sun_dir.y
        );
        sun_cloud_cover *= sun_cover_fade / 10.0;
    }

    // ── Sun ───────────────────────────────────────────────────
    let sun_size_ctrl  = clamp(sky.params2.x, 0.0, 8.0);
    let sun_size       = mix(0.006, 0.055,
        clamp(sun_size_ctrl / 8.0, 0.0, 1.0)) * 3.0;
    let sun_elevation  = dot(ray, sun_dir);
    let sun_visibility = smoothstep(-0.46, -0.42, sun_elevation);

    if (sun_dir.y > -0.34 && sun_elevation > 0.0 && sun_visibility > 0.0) {
        let sun_up  = select(
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
            abs(sun_dir.y) > 0.95
        );
        let sun_tan   = normalize(cross(sun_up, sun_dir));
        let sun_bit   = normalize(cross(sun_dir, sun_tan));
        let sun_local = vec2<f32>(dot(ray, sun_tan), dot(ray, sun_bit))
                      / max(sun_size, 1.0e-4);
        let sun_r     = length(sun_local);

        let sun_vis            = smoothstep(-0.30, -0.06, sun_dir.y);
        let twilight_size_boost = 1.0 + evening_t_raw * 0.55;
        let sun_growth         = mix(0.52, 0.82, sun_vis) * twilight_size_boost;
        let sun_pulse          = 1.0 + 0.035 * sin(sky_time * 1.25);
        let sun_radius_anim    = sun_growth * sun_pulse;
        let sun_blur           = 0.22;
        let sun_r_anim         = sun_r / max(sun_radius_anim, 0.20);
        let cloud_diffusion    = mix(sun_blur, sun_blur * 3.5, sun_cloud_cover);

        // Clouds dim and blur the sun disk — power curve so partial cover
        let cloud_dim  = mix(1.0, 0.1, pow(sun_cloud_cover, 2.4));
        var sun_amount = clamp((1.0 - sun_r_anim) / cloud_diffusion, 0.0, 1.0);
        sun_amount    *= mix(1.0, 0.38, sun_cloud_cover) * cloud_dim;

        if (sun_r < 4.5 && sun_vis > 0.0) {
            let sun_day_col        = vec3<f32>(7.8,  6.9, 1.35);
            let sun_set_col_strong = vec3<f32>(16.0, 3.8, 1.0);
            let sun_set_col_dawn   = vec3<f32>(11.4, 7.3, 3.1);
            let sun_set_col = mix(sun_set_col_strong, sun_set_col_dawn,
                sunrise_twilight * 0.92);
            var sun_col = mix(sun_day_col, sun_set_col,
                pow(evening_t_raw, 0.72) * mix(1.0, 0.72, sunrise_twilight));
            sun_col  = color_saturate(sun_col, mix(1.0, 0.74, sunrise_twilight));
            sun_col *= 1.0 + evening_t_raw * mix(0.25, 0.15, sunrise_twilight);

            // Dim sun colour itself when heavily cloud-occluded
            sun_col *= mix(1.0, 0.22, pow(sun_cloud_cover, 1.2));

            sun_amount  = clamp(sun_amount * (1.0 - moon_amount), 0.0, 1.0);
            sun_amount *= mix(1.0, 1.0 - horizon_amount, 0.25) * sun_visibility;
            let sun_core_amount = sun_amount * 0.36;

            if (sun_col.r > 1.0 || sun_col.g > 1.0 || sun_col.b > 1.0) {
                sun_col *= sun_core_amount;
            }
            color = mix(color, sun_col, sun_core_amount);

            let halo_vis    = smoothstep(-0.02, 0.25, sun_dir.y);
            let halo_growth = mix(0.35, 1.0, halo_vis);
            let sun_r_halo  = sun_r_anim / max(halo_growth, 0.15);
            let core_bloom  = exp(-sun_r_halo * sun_r_halo * 2.7);
            let mid_bloom   = exp(-sun_r_halo * sun_r_halo * 0.88);
            let far_bloom   = exp(-sun_r_halo * sun_r_halo * 0.22);
            let bloom_noise = 0.97 + 0.03 * fbm2(
                sun_local * 2.5
                + vec2<f32>(sky_time * 0.04, -sky_time * 0.03));
            let twinkle  = 0.96 + 0.04
                * sin(sky_time * 2.0
                + atan2(sun_local.y, sun_local.x) * 1.6);
            let flourish = 0.96 + 0.04 * sin(sky_time * 1.2);
            let flare = (core_bloom * 0.52 + mid_bloom * 0.72 + far_bloom * 0.55)
                    * bloom_noise * twinkle * flourish;
            let flare_col = mix(
                vec3<f32>(1.0, 0.84, 0.40),
                vec3<f32>(1.0, 0.62, 0.34),
                evening_t
            );
            let flare_amt = clamp(
                flare * day_t * halo_vis * (1.0 - horizon_amount * 0.75)
                    * 0.52
                    * mix(1.0, 0.46, style_blend_global)
                    * mix(1.0, 0.18, sun_cloud_cover),
                0.0, 0.8  // was 2.0
            );
            color += flare_col * flare_amt * sun_visibility;
        }
    }

    // ── Atmospheric scatter: sky glow in front of cloud-occluded sun ──
    {
        let scatter_angle = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let scatter_wide  = exp(-max(1.0 - scatter_angle, 0.0) * 2.8) * 0.30;
        let scatter_tight = pow(scatter_angle, 14.0) * 0.55;
        let scatter_total = (scatter_wide + scatter_tight)
                          * sun_cloud_cover * day_t * sun_visibility;
        let scatter_col = mix(
            vec3<f32>(0.95, 0.90, 0.72),
            vec3<f32>(1.00, 0.62, 0.38),
            evening_t
        );
        color += scatter_col * scatter_total;
    }

    // ── Stars ─────────────────────────────────────────────────
    let star_size    = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0,  1.0);
    let star_gleam   = clamp(sky.params1.z, 0.0,  2.0);

    if (ray.y > -0.01 && day_t < 0.5) {
        let star_density = mix(0.0045, 0.018, star_scatter);
        let scale_a      = mix(180.0, 360.0, star_scatter);
        let scale_b      = scale_a * 1.73;
        let point_size   = clamp((star_size - 0.2) / 1.8, 0.0, 1.0);
        let pole_fade    = 1.0 - smoothstep(0.90, 0.995, abs(ray.y));

        let stars_a = star_point_field_triplanar(
            ray + vec3<f32>(0.21, -0.15, 0.08), scale_a,
            star_density, point_size);
        let stars_b = star_point_field_triplanar(
            ray + vec3<f32>(-0.17, 0.13, -0.27), scale_b,
            star_density * 0.42, point_size * 0.85);
        var stars = clamp(stars_a + stars_b, 0.0, 1.0);
        stars *= (1.0 - moon_amount) * pole_fade;

        let stars_uv = dir_to_octa_uv(ray);
        let star_seed   = hash12(floor(stars_uv * scale_a));
        let twinkle     = 0.82 + 0.18
            * sin(cloud_time_seconds * 0.65 * (2.2 + star_seed * 9.0));
        let gleam_boost = 1.0 + star_gleam * 1.35;
        let night_alpha = pow(night_t, 3.1);
        let star_band   = smoothstep(0.04, 0.20, ray.y);
        let star_col    = mix(
            vec3<f32>(0.72, 0.78, 1.0),
            vec3<f32>(1.0,  0.97, 0.89),
            star_seed
        );
        color += star_col * stars * night_alpha * star_band * twinkle * gleam_boost;
    }

    // ── Clouds ────────────────────────────────────────────────
