#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sparse_keyframes_and_events() {
        let src = r#"
[Animation]
name = "AttackA"
fps = 30
[/Animation]

[Objects]
@Player = MeshInstance3D
[/Objects]

[Frame0]
@Player {
    position = (0,0,0)
    rotation = (0,0,0,1)
    scale = (1,1,1)
    visible = true
}
[/Frame0]

[Frame25]
@Player {
    call_method = { name="slash", params=[1.0] }
}
[/Frame25]
"#;

        let clip = parse_panim(src).expect("expected valid panim");
        assert_eq!(clip.name.as_ref(), "AttackA");
        assert_eq!(clip.fps, 30.0);
        assert_eq!(clip.total_frames, 26);
        assert_eq!(clip.objects.len(), 1);
        assert_eq!(clip.object_tracks.len(), 2);
        assert_eq!(clip.frame_events.len(), 1);
        assert_eq!(clip.frame_events[0].frame, 25);
    }
}
