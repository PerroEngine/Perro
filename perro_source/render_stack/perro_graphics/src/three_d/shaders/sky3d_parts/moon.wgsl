// ── Moon ──────────────────────────────────────────────────
    var moon_amount    = 0.0;
    let moon_size_ctrl = clamp(sky.params2.y, 0.0, 5.0);
    let moon_size      = mix(0.004, 0.040, clamp(moon_size_ctrl / 5.0, 0.0, 1.0)) * 2.8;
    let moon_dir_n     = normalize(moon_dir);

    // ── Moon cloud cover ──────────────────────────────────────
    var moon_cloud_cover = 0.0;
    if (moon_dir.y > CLOUD_BASE_HEIGHT && day_t < 0.8) {
        let moon_uv_y      = max(moon_dir.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let moon_sky_uv    = moon_dir.xz / sqrt(moon_uv_y);
        let cloud_clock_m  = sky_time
            * (0.011 + length(vec2<f32>(sky.wind.x, sky.wind.y)) * 0.06);
        let wind_s_m       = vec2<f32>(sky.wind.x, sky.wind.y);
        let wind_dir_m     = wind_s_m / max(length(wind_s_m), 1.0e-4);
        let cloud_size_m   = clamp(sky.params0.x, 0.0, 1.0);
        let clouds_scale_m = mix(1.55, 0.55, cloud_size_m);
        let drift_m        = wind_dir_m * cloud_clock_m * 1.12
                           + vec2<f32>(-0.014, -0.005);
        let moon_cloud_uv  = (moon_sky_uv + drift_m) * clouds_scale_m;
        let moon_cloud_raw     = perlin_fbm(moon_cloud_uv, 2);
        let cloud_density_m    = clamp(sky.params0.y, 0.0, 1.0);
        let clouds_cutoff_m    = mix(0.8, 0.52, cloud_density_m);
        let cloud_variance_m   = clamp(sky.params0.z, 0.0, 1.0);
        let clouds_fuzziness_m = mix(0.035, 0.28, cloud_variance_m);
        moon_cloud_cover = smoothstep(
            clouds_cutoff_m - 0.08,
            clouds_cutoff_m + clouds_fuzziness_m + 0.12,
            moon_cloud_raw + cloud_density_m * 0.10
        );
        let moon_cover_fade = smoothstep(
            CLOUD_BASE_HEIGHT,
            CLOUD_BASE_HEIGHT + 0.2,
            moon_dir.y
        );
        moon_cloud_cover *= moon_cover_fade;
    }

    if (day_t < 0.8) {
        let moon_dist = distance(ray, moon_dir_n) / moon_size;
        let moon_blur = 0.2;
        moon_amount   = clamp((1.0 - moon_dist) / moon_blur, 0.0, 1.0);

        // Fade moon when clouds cover it
        let moon_cloud_fade = 1.0 - clamp(moon_cloud_cover * 0.65, 0.0, 0.85);
        moon_amount *= moon_cloud_fade;

        if (moon_amount > 0.0) {
            let moon_ref_a     = vec3<f32>(1.0, 0.0, 0.0);
            let moon_ref_b     = vec3<f32>(0.0, 0.0, 1.0);
            let moon_ref_blend = smoothstep(0.82, 0.98,
                abs(dot(moon_ref_a, moon_dir_n)));
            let moon_ref = normalize(mix(moon_ref_a, moon_ref_b, moon_ref_blend));
            let moon_tan = normalize(
                moon_ref - moon_dir_n * dot(moon_ref, moon_dir_n));
            let moon_bit   = normalize(cross(moon_dir_n, moon_tan));
            let moon_local = vec2<f32>(dot(ray, moon_tan), dot(ray, moon_bit))
                           / max(moon_size, 1.0e-4);

            // ── Sphere surface normal calculation ──
            let moon_radius_sq = dot(moon_local, moon_local);
            
            // Only continue if we're within the sphere
            if (moon_radius_sq < 1.0) {
                // Calculate the Z component to get 3D position on sphere surface
                let moon_z = sqrt(max(1.0 - moon_radius_sq, 0.0));
                let moon_surface_pos = vec3<f32>(moon_local.x, moon_local.y, moon_z);
                let moon_normal = normalize(moon_surface_pos);
                
                // Sun direction for lighting (opposite of moon - assumes sun lights the moon)
                // You may want to use an actual sun_dir variable if available
                let sun_to_moon = normalize(vec3<f32>(-0.3, 0.5, 1.0)); // Adjust as needed
                
                // Lambertian diffuse lighting on sphere
                let moon_ndotl = max(dot(moon_normal, sun_to_moon), 0.0);
                let sphere_lighting = pow(moon_ndotl, 0.8); // Slight gamma for softer falloff

                let moon_fbm_base   = fbm2(moon_local * 2.8 + vec2<f32>(7.0, -11.0));
                let moon_fbm_detail = fbm2(moon_local * 5.4 + vec2<f32>(-3.0, 13.0));
                let crater_seed  = moon_fbm_detail * 0.58 + moon_fbm_base * 0.42;
                let crater_basin = smoothstep(0.44, 0.80, crater_seed);
                let maria        = smoothstep(0.38, 0.70, moon_fbm_base);
                let crater_mask  = clamp(crater_basin * 0.98 + maria * 0.40, 0.0, 1.0);

                // Base moon color with crater details
                let moon_base_col = vec3<f32>(0.90, 0.91, 0.94)
                             - vec3<f32>(0.42, 0.40, 0.44) * crater_mask;
                
                // Apply spherical lighting to moon surface
                // Add ambient term so dark side isn't completely black
                let ambient = 0.08;
                let lit_moon_col = moon_base_col * (ambient + (1.0 - ambient) * sphere_lighting);
                
                // Add subtle depth to craters based on lighting
                let crater_depth = crater_mask * (1.0 - sphere_lighting * 0.3);
                let final_moon_col = lit_moon_col - vec3<f32>(0.12, 0.10, 0.08) * crater_depth;
                
                color = mix(color, final_moon_col, moon_amount);

                // Enhanced rim lighting that respects sphere curvature
                let moon_rim = 1.0 - smoothstep(0.76, 1.32, moon_dist);
                let rim_strength = moon_rim * moon_amount * 0.12;
                
                // Rim should be brighter on lit side
                let oriented_rim = rim_strength * (0.4 + 0.6 * sphere_lighting);
                let moon_rim_col = vec3<f32>(0.80, 0.84, 1.0);
                color += moon_rim_col * oriented_rim;
            }
        }
    }