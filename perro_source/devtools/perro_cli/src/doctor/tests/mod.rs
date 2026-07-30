use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project() -> PathBuf {
    // Nanos alone collide on platforms w/ coarse SystemTime tick (macOS):
    // parallel tests land on same dir + cross-contaminate scans. Add pid +
    // atomic counter -> unique per call regardless of clock granularity.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test setup/result must succeed")
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("perro_cli_doctor_test_{stamp}_{pid}_{seq}"));
    fs::create_dir_all(&dir).expect("test setup/result must succeed");
    dir
}

#[test]
fn script_ref_missing_warns_and_existing_ref_stays_clean() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(project.join("res/existing.png"), b"x").expect("test setup/result must succeed");
    let source = project.join("res/scripts/main.rs");
    fs::write(
        &source,
        r#"
            const OK: &str = "res://existing.png";
            const MISSING: &str = "res://missing.png";
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("script ref missing"));
    assert!(report.messages[0].contains("res://scripts/main.rs:3"));
    assert!(report.messages[0].contains("res://missing.png"));
    assert!(!report.messages[0].contains(&project.to_string_lossy().to_string()));
}

#[test]
fn doctor_ignores_scene_refs_in_comments() {
    let project = temp_project();
    fs::create_dir_all(project.join("res")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/main.scn"),
        r##"
            # texture = "res://missing_hash.png"
            // texture = "res://missing_slash.png"
            color = "#ffeeaa"
            "##,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    let mut files = Vec::new();
    collect_reference_text_files(&project, &mut files).expect("test setup/result must succeed");
    for file in files {
        let text = fs::read_to_string(&file).expect("test setup/result must succeed");
        for text_ref in extract_virtual_refs(&text) {
            validate_virtual_ref(
                &project,
                Some(&file),
                Some(text_ref.line),
                &text_ref.raw,
                &mut report,
            );
        }
    }

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
}

#[test]
fn doctor_ignores_script_refs_and_macros_in_comments() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/main.rs"),
        r#"
            #[State]
            struct PlayerState {
                hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                // let _ = get_var!(ctx.run, ctx.id, var!("missing_hp"));
                // let _ = "res://missing_script_ref.png";
                /*
                set_var!(ctx.run, ctx.id, var!("missing_flag"), variant!(true));
                let _ = "res://missing_block_ref.png";
                */
            }
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
}

#[test]
fn script_ref_dlc_self_resolves_from_source_dlc_root() {
    let project = temp_project();
    fs::create_dir_all(project.join("dlcs/cosmetic/scripts"))
        .expect("test setup/result must succeed");
    fs::create_dir_all(project.join("dlcs/cosmetic/textures"))
        .expect("test setup/result must succeed");
    fs::write(project.join("dlcs/cosmetic/textures/hat.png"), b"x")
        .expect("test setup/result must succeed");
    fs::write(
        project.join("dlcs/cosmetic/scripts/main.rs"),
        r#"const HAT: &str = "dlc://self/textures/hat.png";"#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
    assert_eq!(report.checked_refs, 1);
}

#[test]
fn script_index_reads_state_fields_and_methods_macro() {
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/main.rs"),
        r#"
            #[State]
            pub struct PlayerState {
                pub hp: i32,
                energy: f32,
            }

            methods!({
                fn heal(
                    &self,
                    ctx: &mut ScriptContext<'_, API>,
                    amount: i32,
                ) {}

                pub fn ping(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
            "#,
        &mut index,
    );

    assert!(index.state_fields.contains("hp"));
    assert!(index.state_fields.contains("energy"));
    assert!(index.methods.contains_key("heal"));
    assert!(index.methods.contains_key("ping"));
    // vis tracked per def: heal !pub, ping pub
    assert!(!index.methods["heal"].is_pub);
    assert!(index.methods["ping"].is_pub);
    let hp_defs = &index.state_field_defs["hp"];
    assert!(hp_defs.iter().all(|def| def.is_pub));
    let energy_defs = &index.state_field_defs["energy"];
    assert!(energy_defs.iter().all(|def| !def.is_pub));
}

#[test]
fn script_index_keeps_methods_after_res_url_opening_a_block() {
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/main.rs"),
        r#"
            #[State]
            pub struct PlayerState {
                pub hp: i32,
            }

            methods!({
                fn alpha(&self, ctx: &mut ScriptContext<'_, API>) {
                    if ctx.scene_path() == "res://ui/raw_export.scn" {
                        let _ = ctx.id;
                    }
                }

                fn beta(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
            "#,
        &mut index,
    );

    assert!(index.methods.contains_key("alpha"));
    assert!(index.methods.contains_key("beta"));
}

#[test]
fn script_index_ignores_braces_in_char_literals() {
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/main.rs"),
        r#"
            methods!({
                fn alpha(&self, ctx: &mut ScriptContext<'_, API>) {
                    let open = '{';
                    let quote = '"';
                    let _ = (open, quote, ctx.id);
                }

                fn beta(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
            "#,
        &mut index,
    );

    assert!(index.methods.contains_key("alpha"));
    assert!(index.methods.contains_key("beta"));
}

#[test]
fn script_index_ignores_braces_in_block_comments() {
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/main.rs"),
        r#"
            methods!({
                fn alpha(&self, ctx: &mut ScriptContext<'_, API>) {
                    /* stray } brace + fn ghost( in block comment */
                    let _ = ctx.id;
                }

                fn beta(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
            "#,
        &mut index,
    );

    assert!(index.methods.contains_key("alpha"));
    assert!(index.methods.contains_key("beta"));
}

#[test]
fn script_member_checks_warn_missing_members_and_ctx_id_hints() {
    let file = PathBuf::from("res/scripts/main.rs");
    let text = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, ctx.id, var!("missing_hp"));
                set_var!(ctx.run, ctx.id, "missing_flag", variant!(true));
                let _ = call_method!(ctx.run, ctx.id, method!("missing_method"), params![]);
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index.state_fields.insert("hp".to_string());
    index
        .state_field_owners
        .insert("hp".to_string(), "PlayerState".to_string());
    index.methods.insert(
        "heal".to_string(),
        DoctorMethodDef {
            is_pub: true,
            dispatch: true,
            files: Vec::new(),
            pub_files: Vec::new(),
            dispatch_pub_files: Vec::new(),
        },
    );
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, text, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 6);
    assert!(report.messages.iter().any(|m| m.contains("with_state!")));
    assert!(
        report
            .messages
            .iter()
            .any(|m| m.contains("with_state_mut!"))
    );
    assert!(report.messages.iter().any(|m| m.contains(
        "with_state_mut!(ctx.run, StateType, ctx.id, |state| state.missing_flag = variant!(true))"
    )));
    assert!(
        report
            .messages
            .iter()
            .any(|m| m.contains("self.missing_method(ctx, params...)"))
    );
    assert!(report.messages.iter().any(|m| m.contains("missing_hp")));
    assert!(report.messages.iter().any(|m| m.contains("missing_flag")));
    assert!(report.messages.iter().any(|m| m.contains("missing_method")));
}

#[test]
fn script_state_access_checks_require_state_attribute() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            #[State]
            struct PlayerState {
                hp: i32,
            }

            struct HelperState {
                hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = with_state!(ctx.run, PlayerState, ctx.id, |state| state.hp);
                let _ = with_state!(ctx.run, crate::PlayerState, ctx.id, |state| state.hp);
                let _ = with_state_mut!(ctx.run, HelperState, ctx.id, |state| state.hp += 1);
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/main.rs"), source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("script state missing"));
    assert!(report.messages[0].contains("with_state_mut!"));
    assert!(report.messages[0].contains("HelperState"));
    assert!(!report.messages[0].contains("PlayerState"));
}

#[test]
fn script_member_checks_walk_nested_custom_state_fields() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            pub struct Aim {
                pub axis: Axis,
            }

            pub enum Axis {
                Local { x: f32, y: f32 },
                World {
                    dir: Direction,
                },
            }

            pub struct Direction {
                pub yaw: f32,
            }

            #[State]
            pub struct SpinnerState {
                pub aim: Aim,
                pub hp: i32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, other, var!("aim.axis.dir.yaw"));
                let _ = get_var!(ctx.run, other, var!("aim.axis.dir.pitch"));
                let _ = get_var!(ctx.run, other, var!("hp.value"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/main.rs"), source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 2);
    assert!(
        report
            .messages
            .iter()
            .any(|m| m.contains("aim.axis.dir.pitch"))
    );
    assert!(report.messages.iter().any(|m| m.contains("hp.value")));
    assert!(
        report
            .messages
            .iter()
            .all(|m| !m.contains("aim.axis.dir.yaw"))
    );
}

#[test]
fn script_member_checks_accept_shared_state_field_name_across_scripts() {
    let file = PathBuf::from("res/scripts/golf_manager.rs");
    let golf_source = r#"
            pub struct GolfAgentConfigState {
                pub club_index: i32,
                pub right_handed: bool,
                pub orbit_yaw_degrees: f32,
            }

            #[State]
            pub struct GolfAgentState {
                pub config: GolfAgentConfigState,
            }
        "#;
    let volleyball_source = r#"
            pub struct VolleyballConfigState {
                pub serve_power: f32,
            }

            #[State]
            pub struct VolleyballAgentState {
                pub config: VolleyballConfigState,
            }
        "#;
    let caller_source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, agent, var!("config.club_index"));
                let _ = get_var!(ctx.run, agent, var!("config.right_handed"));
                let _ = get_var!(ctx.run, agent, var!("config.serve_power"));
                let _ = get_var!(ctx.run, agent, var!("config.missing_member"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/golf_agent.rs"),
        golf_source,
        &mut index,
    );
    index_script_source(
        Path::new("res/scripts/volleyball_agent.rs"),
        volleyball_source,
        &mut index,
    );
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("config.missing_member"));

    // same lookups stay valid when scripts index in the opposite order
    let mut index = ScriptDoctorIndex::default();
    index_script_source(
        Path::new("res/scripts/volleyball_agent.rs"),
        volleyball_source,
        &mut index,
    );
    index_script_source(
        Path::new("res/scripts/golf_agent.rs"),
        golf_source,
        &mut index,
    );
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("config.missing_member"));
}

#[test]
fn script_member_checks_warn_private_members() {
    let file = PathBuf::from("res/scripts/caller.rs");
    let target_source = r#"
            #[State]
            pub struct EnemyState {
                hp: i32,
                pub armor: i32,
            }

            methods!({
                fn hide_internal(&self, ctx: &mut ScriptContext<'_, API>) {}

                pub fn heal(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
        "#;
    let caller_source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, enemy, var!("hp"));
                let _ = get_var!(ctx.run, enemy, var!("armor"));
                let _ = call_method!(ctx.run, enemy, func!("hide_internal"), params![]);
                let _ = call_method!(ctx.run, enemy, func!("heal"), params![]);
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/enemy.rs"), target_source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 2, "messages: {:?}", report.messages);
    assert!(report.messages.iter().any(|m| {
        m.contains("script member private")
            && m.contains("`hp`")
            && m.contains("res://scripts/enemy.rs")
            && m.contains("make the field `pub`")
    }));
    assert!(report.messages.iter().any(|m| {
        m.contains("script member private")
            && m.contains("`hide_internal`")
            && m.contains("res://scripts/enemy.rs")
            && m.contains("make it `pub fn`")
    }));
    assert!(report.messages.iter().all(|m| !m.contains("`armor`")));
    assert!(report.messages.iter().all(|m| !m.contains("`heal`")));
}

#[test]
fn signal_connect_private_handler_warns() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            methods!({
                fn on_click(&self, ctx: &mut ScriptContext<'_, API>) {}

                pub fn on_hover(&self, ctx: &mut ScriptContext<'_, API>) {}
            });

            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_connect!(ctx.run, ctx.id, signal!("clicked"), func!("on_click"));
                let _ = signal_connect!(ctx.run, ctx.id, signal!("hovered"), func!("on_hover"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(&file, source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("script member private"));
    assert!(report.messages[0].contains("`on_click`"));
    assert!(report.messages[0].contains("signal_connect!"));
}

#[test]
fn signal_connect_pairs_checks_handlers_and_counts_connects() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            methods!({
                fn on_play(&self, ctx: &mut ScriptContext<'_, API>) {}

                pub fn on_quit(&self, ctx: &mut ScriptContext<'_, API>) {}
            });

            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_emit!(ctx.run, signal!("play_clicked"));
                let _ = signal_connect_pairs!(
                    ctx.run,
                    ctx.id,
                    [
                        ("play_clicked", "on_play"),
                        ("quit_clicked", "on_quit"),
                    ]
                );
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(&file, source, &mut index);
    index_script_signal_uses(&file, source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, source, &index, &mut report);
    validate_signal_emits(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    // 1 private handler warn; pairs connect covers the emit -> no unused warn
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("script member private"));
    assert!(report.messages[0].contains("`on_play`"));
    assert!(report.messages[0].contains("signal_connect_pairs!"));
}

#[test]
fn unused_pub_members_warn_and_dynamic_uses_suppress() {
    let target_source = r#"
            #[State]
            pub struct EnemyState {
                pub hp: i32,
                pub armor: i32,
                cooldown: f32,
            }

            pub struct SortRules;

            impl SortRules {
                pub fn len(&self) -> usize {
                    0
                }

                pub fn pick(
                    &self,
                    index: usize,
                ) -> usize {
                    index
                }
            }

            methods!({
                pub fn heal(&self, ctx: &mut ScriptContext<'_, API>) {}

                pub fn taunt(&self, ctx: &mut ScriptContext<'_, API>) {}

                fn plan(&self, ctx: &mut ScriptContext<'_, API>) {}
            });
        "#;
    let caller_source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, enemy, var!("hp"));
                let _ = call_method!(ctx.run, enemy, func!("heal"), params![]);
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/enemy.rs"), target_source, &mut index);
    index_script_source(
        Path::new("res/scripts/caller.rs"),
        caller_source,
        &mut index,
    );
    let mut report = ValidationReport::default();

    validate_unused_pub_members(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    // hp + heal used; cooldown + plan not pub; armor + taunt unused pub
    assert_eq!(report.warnings, 2, "messages: {:?}", report.messages);
    assert!(report.messages.iter().any(|m| {
        m.contains("script member unused pub")
            && m.contains("`armor`")
            && m.contains("res://scripts/enemy.rs")
    }));
    assert!(
        report
            .messages
            .iter()
            .any(|m| { m.contains("script member unused pub") && m.contains("`taunt`") })
    );
    assert!(report.messages.iter().all(|m| !m.contains("`hp`")));
    assert!(report.messages.iter().all(|m| !m.contains("`heal`")));
    assert!(report.messages.iter().all(|m| !m.contains("`cooldown`")));
    assert!(report.messages.iter().all(|m| !m.contains("`plan`")));
    // plain helper fns w/o ctx get no glue -> pub on them is free, no warn
    assert!(report.messages.iter().all(|m| !m.contains("`len`")));
    assert!(report.messages.iter().all(|m| !m.contains("`pick`")));
}

#[test]
fn call_method_on_ctx_less_helper_warns_not_callable() {
    let file = PathBuf::from("res/scripts/caller.rs");
    let target_source = r#"
            pub struct Rules;

            impl Rules {
                pub fn total(&self) -> i32 {
                    0
                }
            }
        "#;
    let caller_source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = call_method!(ctx.run, rules, func!("total"), params![]);
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/rules.rs"), target_source, &mut index);
    let mut report = ValidationReport::default();

    validate_script_member_calls(Path::new(""), &file, caller_source, &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("script member not callable"));
    assert!(report.messages[0].contains("`total`"));
    assert!(report.messages[0].contains("ScriptContext"));
}

#[test]
fn unused_pub_suppressed_by_signal_handlers_nested_vars_and_shared_names() {
    let source = r#"
            #[State]
            pub struct WalkerState {
                pub config: WalkerConfig,
                pub speed: f32,
            }

            methods!({
                pub fn on_step(&self, ctx: &mut ScriptContext<'_, API>) {}
            });

            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_connect!(ctx.run, ctx.id, signal!("step"), func!("on_step"));
                let _ = get_var!(ctx.run, other, var!("config.gain"));
            }
        "#;
    // 2nd script declares same-name pub field; caller's use of `speed`
    // cannot be attributed -> both stay quiet (no dedup on shared names)
    let other_source = r#"
            #[State]
            pub struct RunnerState {
                pub speed: f32,
            }

            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = get_var!(ctx.run, walker, var!("speed"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_source(Path::new("res/scripts/walker.rs"), source, &mut index);
    index_script_source(Path::new("res/scripts/runner.rs"), other_source, &mut index);
    let mut report = ValidationReport::default();

    validate_unused_pub_members(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    // on_step = signal handler use; config = nested-root use; speed = shared use
    assert_eq!(report.warnings, 0, "messages: {:?}", report.messages);
}

#[test]
fn resource_member_event_names_count_as_uses() {
    let text = r#"
            [Frame10]
            @Hero {
                set_var = { name="combo", value=2 }
                call_method = { name="spawn_trail", params=[0.2] }
            }
        "#;

    let names = extract_resource_member_event_names(text);

    assert_eq!(names, vec!["combo".to_string(), "spawn_trail".to_string()]);
}

#[test]
fn signal_emit_without_connect_warns_once_per_emit_location() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            fn run(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_emit!(ctx, signal!("loose_signal"));
                let _ = signal_emit!(ctx, signal!("wired_signal"), params![1_i32]);
                let _ = signal_connect!(ctx, ctx.id, signal!("wired_signal"), func!("on_wired"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_signal_uses(&file, source, &mut index);
    let mut report = ValidationReport::default();

    validate_signal_emits(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("signal: loose_signal"));
    assert!(report.messages[0].contains("never connected anywhere"));
    assert!(!report.messages[0].contains("wired_signal"));
}

#[test]
fn signal_connect_without_emit_stays_clean() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_connect!(ctx, ctx.id, signal!("future_button_click"), func!("on_click"));
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_signal_uses(&file, source, &mut index);
    let mut report = ValidationReport::default();

    validate_signal_emits(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
}

#[test]
fn signal_connect_many_counts_as_connected() {
    let file = PathBuf::from("res/scripts/main.rs");
    let source = r#"
            fn ready(ctx: &mut ScriptContext<'_, API>) {
                let _ = signal_emit!(ctx, signal!("wired_a"));
                let _ = signal_emit!(ctx, signal!("wired_b"));
                let _ = signal_connect_many!(
                    ctx,
                    ctx.id,
                    [signal!("wired_a"), signal!("wired_b")],
                    [func!("on_signal")]
                );
            }
        "#;
    let mut index = ScriptDoctorIndex::default();
    index_script_signal_uses(&file, source, &mut index);
    let mut report = ValidationReport::default();

    validate_signal_emits(Path::new(""), &index, &mut report);

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 0);
}

#[test]
fn resource_signal_fields_count_as_emits() {
    let text = r#"
            clicked_signals = ["play_clicked", "any_button_clicked"]
            emit_signal = { name="step", params=[0] }
        "#;

    let refs = extract_resource_signal_emits(text);

    assert_eq!(
        refs.iter().map(|r| r.raw.as_str()).collect::<Vec<_>>(),
        vec!["play_clicked", "any_button_clicked", "step"]
    );
}

#[test]
fn resource_signal_emit_warns_without_scripts() {
    let project = temp_project();
    fs::create_dir_all(project.join("res")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/ui.scn"),
        r#"clicked_signals = ["play_clicked"]"#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1);
    assert!(report.messages[0].contains("signal: play_clicked"));
    assert!(report.messages[0].contains("res://ui.scn:1"));
}

#[test]
fn node_ref_type_hints_warn_for_script_vars_and_builtin_fields() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/player.rs"),
        r#"
            use perro_api::prelude::*;

            #[State]
            pub struct PlayerState {
                #[expose]
                #[node_ref(Camera3D)]
                pub camera: NodeID,
            }
            "#,
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/main.scn"),
        r#"
            $root = @Player

            [Player]
            script = "res://scripts/player.rs"
            script_vars = { camera = @Mesh }
            [Node3D/]
            [/Player]

            [Stream]
            [UiCameraStream]
                camera = @Mesh
            [/UiCameraStream]
            [/Stream]

            [Mesh]
            [MeshInstance3D/]
            [/Mesh]
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 2);
    assert!(
        report
            .messages
            .iter()
            .any(|msg| msg.contains("Player.script_vars.camera wants Node(Camera3D)"))
    );
    assert!(
        report
            .messages
            .iter()
            .any(|msg| msg.contains("Stream.camera wants Node(Camera2D|Camera3D|Webcam)"))
    );
}

#[test]
fn node_ref_hints_resolve_by_attached_script_for_shared_field_names() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/golf_agent.rs"),
        r#"
            use perro_api::prelude::*;

            pub struct GolfConfig {
                #[expose]
                #[node_ref(Camera3D)]
                pub orbit_camera: NodeID,
            }

            #[State]
            pub struct GolfAgentState {
                #[expose]
                pub config: GolfConfig,
            }
            "#,
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/volleyball_agent.rs"),
        r#"
            use perro_api::prelude::*;

            pub struct VolleyballConfig {
                #[expose]
                pub serve_power: f32,
            }

            #[State]
            pub struct VolleyballAgentState {
                #[expose]
                pub config: VolleyballConfig,
            }
            "#,
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/main.scn"),
        r#"
            $root = @Golfer

            [Golfer]
            script = "res://scripts/golf_agent.rs"
            script_vars = { config = { orbit_camera = @Cam } }
            [Node3D/]
            [/Golfer]

            [BadGolfer]
            script = "res://scripts/golf_agent.rs"
            script_vars = { config = { orbit_camera = @Mesh } }
            [Node3D/]
            [/BadGolfer]

            [Cam]
            [Camera3D/]
            [/Cam]

            [Mesh]
            [MeshInstance3D/]
            [/Mesh]
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0);
    // scene script_vars wiring suppresses unused-pub 4 `config`
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(
        report.messages[0]
            .contains("BadGolfer.script_vars.config.orbit_camera wants Node(Camera3D)")
    );
}

#[test]
fn scene_var_on_private_field_warns() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/walker.rs"),
        r#"
            use perro_api::prelude::*;

            #[State]
            pub struct WalkerState {
                stride: f32,
                pub pace: f32,
            }
            "#,
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/main.scn"),
        r#"
            $root = @Walker

            [Walker]
            script = "res://scripts/walker.rs"
            script_vars = { stride = 1.5, pace = 2.0 }
            [Node3D/]
            [/Walker]
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    // stride private + scene-set -> inject dropped warn; pace pub + scene-set -> clean
    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("scene var private"));
    assert!(report.messages[0].contains("`stride`"));
    assert!(report.messages[0].contains("will not apply"));
    assert!(!report.messages[0].contains("pace"));
}

#[test]
fn unused_pub_field_warns_in_full_pipeline_unless_scene_sets_it() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/scripts")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/scripts/walker.rs"),
        r#"
            use perro_api::prelude::*;

            #[State]
            pub struct WalkerState {
                pub stamina: f32,
                pub stride: f32,
            }
            "#,
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/main.scn"),
        r#"
            $root = @Walker

            [Walker]
            script = "res://scripts/walker.rs"
            script_vars = { stride = 1.5 }
            [Node3D/]
            [/Walker]
            "#,
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_script_warnings(&project, &mut report).expect("test setup/result must succeed");

    // stride scene-set -> quiet; stamina pub + unused -> warn
    assert_eq!(report.errors, 0);
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("script member unused pub"));
    assert!(report.messages[0].contains("`stamina`"));
    assert!(report.messages[0].contains("res://scripts/walker.rs"));
    assert!(!report.messages[0].contains("stride"));
}

#[test]
fn shader_check_reports_broken_material_at_source_line() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/shaders")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/shaders/broken.wgsl"),
        "fn shade_material(in: FragmentInput) -> vec4<f32> {\n    return perro_not_a_helper(in);\n}\n",
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_shaders(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.warnings, 0, "messages: {:?}", report.messages);
    assert_eq!(report.errors, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("res://shaders/broken.wgsl:2:"));
    assert!(!report.messages[0].contains(&project.to_string_lossy().to_string()));
}

#[test]
fn shader_check_accepts_material_sky_and_post_shaders() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/shaders")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/shaders/material.wgsl"),
        "fn shade_material(in: FragmentInput) -> vec4<f32> {\n    return unpack_rgba8(in.packed_color) * custom_f_param(in, 0u);\n}\n",
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/shaders/sky.wgsl"),
        "fn sky_shader(in: SkyFragment) -> vec4<f32> {\n    return vec4<f32>(in.color.rgb * in.day_weight, in.color.a);\n}\n",
    )
    .expect("test setup/result must succeed");
    fs::write(
        project.join("res/shaders/post.wgsl"),
        "fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {\n    return vec4<f32>(color.rgb * linearize_depth(depth), color.a);\n}\n",
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_shaders(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0, "messages: {:?}", report.messages);
    assert_eq!(report.warnings, 0, "messages: {:?}", report.messages);
    assert_eq!(report.checked_files, 3);
}

#[test]
fn shader_without_engine_entry_fn_warns_only() {
    let project = temp_project();
    fs::create_dir_all(project.join("res/shaders")).expect("test setup/result must succeed");
    fs::write(
        project.join("res/shaders/helpers.wgsl"),
        "fn scratch_helper(x: f32) -> f32 { return x * 2.0; }\n",
    )
    .expect("test setup/result must succeed");

    let mut report = ValidationReport::default();
    validate_shaders(&project, &mut report).expect("test setup/result must succeed");

    assert_eq!(report.errors, 0, "messages: {:?}", report.messages);
    assert_eq!(report.warnings, 1, "messages: {:?}", report.messages);
    assert!(report.messages[0].contains("no engine entry fn"));
}
