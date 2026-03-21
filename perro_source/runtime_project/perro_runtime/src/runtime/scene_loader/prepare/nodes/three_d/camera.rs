fn build_camera_3d(data: &SceneDefNodeData) -> Camera3D {
    let mut node = Camera3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_camera_3d_fields(&mut node, &data.fields);
    node
}

fn apply_camera_3d_fields(node: &mut Camera3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "zoom" => {
                if let Some(v) = as_f32(value) {
                    apply_zoom_compat_projection(node, v);
                }
            }
            "projection" => {
                if let Some(v) = as_str(value) {
                    set_projection_mode(node, v);
                }
            }
            "perspective_fov_y_degrees" => {
                if let Some(v) = as_f32(value) {
                    set_projection_fov(node, v);
                }
            }
            "perspective_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_perspective_near(node, v);
                }
            }
            "perspective_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_perspective_far(node, v);
                }
            }
            "orthographic_size" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_size(node, v);
                }
            }
            "orthographic_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_near(node, v);
                }
            }
            "orthographic_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_far(node, v);
                }
            }
            "frustum_left" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_left(node, v);
                }
            }
            "frustum_right" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_right(node, v);
                }
            }
            "frustum_bottom" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_bottom(node, v);
                }
            }
            "frustum_top" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_top(node, v);
                }
            }
            "frustum_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_near(node, v);
                }
            }
            "frustum_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_far(node, v);
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        });
}

fn apply_zoom_compat_projection(node: &mut Camera3D, zoom: f32) {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let fov_y_degrees = (60.0 / zoom).clamp(10.0, 120.0);
    if let CameraProjection::Perspective {
        fov_y_degrees: fov, ..
    } = &mut node.projection
    {
        *fov = fov_y_degrees;
    }
}

fn set_projection_mode(node: &mut Camera3D, mode: &str) {
    match mode {
        "perspective" => {
            node.projection = CameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        "orthographic" => {
            node.projection = CameraProjection::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        "frustum" => {
            node.projection = CameraProjection::Frustum {
                left: -1.0,
                right: 1.0,
                bottom: -1.0,
                top: 1.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        _ => {}
    }
}

fn set_projection_fov(node: &mut Camera3D, value: f32) {
    let fov = value.clamp(10.0, 120.0);
    if let CameraProjection::Perspective { fov_y_degrees, .. } = &mut node.projection {
        *fov_y_degrees = fov;
    }
}

fn set_projection_perspective_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Perspective { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_perspective_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Perspective { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}

fn set_projection_ortho_size(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { size, .. } = &mut node.projection {
        *size = value.abs().max(0.001);
    }
}

fn set_projection_ortho_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_ortho_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}

fn set_projection_frustum_left(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { left, right, .. } = &mut node.projection {
        *left = value;
        if *right <= *left {
            *right = *left + 0.001;
        }
    }
}

fn set_projection_frustum_right(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { left, right, .. } = &mut node.projection {
        *right = value.max(*left + 0.001);
    }
}

fn set_projection_frustum_bottom(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { bottom, top, .. } = &mut node.projection {
        *bottom = value;
        if *top <= *bottom {
            *top = *bottom + 0.001;
        }
    }
}

fn set_projection_frustum_top(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { bottom, top, .. } = &mut node.projection {
        *top = value.max(*bottom + 0.001);
    }
}

fn set_projection_frustum_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_frustum_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}
