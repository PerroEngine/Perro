use super::*;

#[test]
fn connect_dedup_disconnect_emit_snapshot() {
    let signal = SignalID::from_string("on_test");
    let id1 = NodeID::new(1);
    let id2 = NodeID::new(2);
    let f1 = ScriptMemberID::from_string("a");
    let f2 = ScriptMemberID::from_string("b");
    let f3 = ScriptMemberID::from_string("c");

    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, id1, f1, &[]));
    assert!(!reg.connect(signal, id1, f1, &[]));
    assert!(reg.connect(signal, id1, f2, &[perro_variant::Variant::from("right")]));
    assert!(reg.connect(signal, id2, f3, &[]));

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(out.len(), 3);
    assert_eq!(out[1].params(), &[perro_variant::Variant::from("right")]);

    assert!(reg.disconnect(signal, id1, f2));
    assert!(!reg.disconnect(signal, id1, f2));

    out.clear();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(out.len(), 2);
}

#[test]
fn multi_connection_snapshot_shares_storage_until_reentrant_membership_change() {
    let signal = SignalID::from_string("cached_snapshot");
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, NodeID::new(1), method, &[]));
    assert!(reg.connect(signal, NodeID::new(2), method, &[]));

    let Some(SignalConnectionsSnapshot::Multiple(first)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    let Some(SignalConnectionsSnapshot::Multiple(second)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    assert!(Rc::ptr_eq(&first, &second));

    assert!(reg.connect(signal, NodeID::new(3), method, &[]));
    let Some(SignalConnectionsSnapshot::Multiple(changed)) =
        reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    assert!(!Rc::ptr_eq(&first, &changed));
    assert_eq!(first.len(), 2);
    assert_eq!(changed.len(), 3);
}

#[test]
fn multi_connection_storage_stays_inline_through_four_and_spills_at_five() {
    let signal = SignalID::from_string("small_fanout");
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, NodeID::new(1), method, &[]));
    assert!(reg.connect(signal, NodeID::new(2), method, &[]));

    let Some(SignalConnectionsSnapshot::Multiple(two)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    let storage = Rc::as_ptr(&two);
    assert!(!two.spilled());
    drop(two);

    assert!(reg.connect(signal, NodeID::new(3), method, &[]));
    assert!(reg.connect(signal, NodeID::new(4), method, &[]));
    let Some(SignalConnectionsSnapshot::Multiple(four)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    assert_eq!(Rc::as_ptr(&four), storage);
    assert!(!four.spilled());
    drop(four);

    assert!(reg.connect(signal, NodeID::new(5), method, &[]));
    let Some(SignalConnectionsSnapshot::Multiple(five)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    assert_eq!(Rc::as_ptr(&five), storage);
    assert!(five.spilled());
}

#[test]
fn unrelated_teardown_keeps_shared_multi_snapshot_storage() {
    let signal = SignalID::from_string("shared_snapshot");
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, NodeID::new(1), method, &[]));
    assert!(reg.connect(signal, NodeID::new(2), method, &[]));
    let Some(SignalConnectionsSnapshot::Multiple(before)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };

    assert_eq!(reg.disconnect_script(NodeID::new(3)), 0);

    let Some(SignalConnectionsSnapshot::Multiple(after)) = reg.signal_connections_snapshot(signal)
    else {
        panic!("expected multi snapshot");
    };
    assert!(Rc::ptr_eq(&before, &after));
}

#[test]
fn empty_connect_params_use_empty_view() {
    let signal = SignalID::from_string("empty_params");
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, NodeID::new(1), method, &[]));

    let Some(SignalConnectionsSnapshot::Single(connection)) =
        reg.signal_connections_snapshot(signal)
    else {
        panic!("expected single snapshot");
    };
    assert!(connection.params().is_empty());
}

#[test]
fn disconnect_script_removes_all_entries() {
    let s1 = SignalID::from_string("s1");
    let s2 = SignalID::from_string("s2");
    let id1 = NodeID::new(10);
    let id2 = NodeID::new(11);
    let f = ScriptMemberID::from_string("h");

    let mut reg = SignalRegistry::new();
    assert!(reg.connect(s1, id1, f, &[]));
    assert!(reg.connect(s1, id2, f, &[]));
    assert!(reg.connect(s2, id1, f, &[]));
    assert_eq!(reg.disconnect_script(id1), 2);

    let mut out = Vec::new();
    reg.copy_signal_connections(s1, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].script_id, id2);

    out.clear();
    reg.copy_signal_connections(s2, &mut out);
    assert!(out.is_empty());
}

#[test]
fn disconnect_script_without_connections_scans_all_signal_buckets() {
    let other = NodeID::new(1);
    let missing = NodeID::new(2);
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    for i in 0..256 {
        assert!(reg.connect(SignalID::from_u64(i), other, method, &[]));
    }
    reg.reset_disconnect_script_counters();

    assert_eq!(reg.disconnect_script(missing), 0);
    assert_eq!(reg.disconnect_script_counters(), (256, 256));
}

#[test]
fn disconnect_script_scans_all_buckets_and_removes_only_target() {
    let target = NodeID::new(1);
    let other = NodeID::new(2);
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    for i in 0..256 {
        assert!(reg.connect(SignalID::from_u64(i), other, method, &[]));
    }
    for i in [7, 91, 203] {
        let signal = SignalID::from_u64(i);
        assert!(reg.connect(signal, target, method, &[]));
    }
    reg.reset_disconnect_script_counters();

    assert_eq!(reg.disconnect_script(target), 3);
    assert_eq!(reg.disconnect_script_counters(), (256, 259));
}

#[test]
fn disconnect_script_counts_many_methods_per_signal() {
    let target = NodeID::new(1);
    let signal = SignalID::from_string("many_methods");
    let mut reg = SignalRegistry::new();
    for i in 0..64 {
        assert!(reg.connect(signal, target, ScriptMemberID(i), &[]));
    }
    reg.reset_disconnect_script_counters();

    assert_eq!(reg.disconnect_script(target), 64);
    assert_eq!(reg.disconnect_script_counters(), (1, 64));

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert!(out.is_empty());
}

#[test]
fn disconnect_then_script_teardown_removes_remaining_connection() {
    let target = NodeID::new(1);
    let signal = SignalID::from_string("partial_disconnect");
    let first = ScriptMemberID::from_string("first");
    let second = ScriptMemberID::from_string("second");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, target, first, &[]));
    assert!(reg.connect(signal, target, second, &[]));

    assert!(reg.disconnect(signal, target, first));
    assert_eq!(reg.disconnect_script(target), 1);

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert!(out.is_empty());
}

#[test]
fn script_teardown_counts_connections_across_signals() {
    let target = NodeID::new(1);
    let first_signal = SignalID::from_string("first_signal");
    let second_signal = SignalID::from_string("second_signal");
    let first = ScriptMemberID::from_string("first");
    let second = ScriptMemberID::from_string("second");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(first_signal, target, first, &[]));
    assert!(reg.connect(first_signal, target, second, &[]));
    assert!(reg.connect(second_signal, target, first, &[]));
    assert!(reg.disconnect(first_signal, target, first));
    reg.reset_disconnect_script_counters();

    assert_eq!(reg.disconnect_script(target), 2);
    assert_eq!(reg.disconnect_script_counters(), (2, 2));
}

#[test]
fn duplicate_connect_and_failed_disconnect_leave_connection_live() {
    let target = NodeID::new(1);
    let signal = SignalID::from_string("dedup_count");
    let method = ScriptMemberID::from_string("handle");
    let missing = ScriptMemberID::from_string("missing");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, target, method, &[]));
    assert!(!reg.connect(signal, target, method, &[]));
    assert!(!reg.disconnect(signal, target, missing));

    assert_eq!(reg.disconnect_script(target), 1);
}

#[test]
fn partial_remove_and_readd_keep_teardown_correct() {
    let target = NodeID::new(1);
    let first_signal = SignalID::from_string("first_signal");
    let second_signal = SignalID::from_string("second_signal");
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(first_signal, target, method, &[]));
    assert!(reg.connect(second_signal, target, method, &[]));
    assert!(reg.disconnect(second_signal, target, method));
    assert!(reg.connect(second_signal, target, method, &[]));
    assert!(reg.disconnect(first_signal, target, method));
    reg.reset_disconnect_script_counters();

    assert_eq!(reg.disconnect_script(target), 1);
    assert_eq!(reg.disconnect_script_counters(), (1, 1));
}

#[test]
fn disconnect_keeps_swap_remove_order() {
    let signal = SignalID::from_string("swap_remove");
    let ids = [NodeID::new(1), NodeID::new(2), NodeID::new(3)];
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    for id in ids {
        assert!(reg.connect(signal, id, method, &[]));
    }

    assert!(reg.disconnect(signal, ids[0], method));

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(
        out.iter()
            .map(|connection| connection.script_id)
            .collect::<Vec<_>>(),
        [ids[2], ids[1]]
    );
}

#[test]
fn teardown_keeps_survivor_order_and_emission_snapshot() {
    let signal = SignalID::from_string("snapshot");
    let target = NodeID::new(1);
    let survivors = [NodeID::new(2), NodeID::new(3), NodeID::new(4)];
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, survivors[0], method, &[]));
    assert!(reg.connect(signal, target, ScriptMemberID(1), &[]));
    assert!(reg.connect(signal, survivors[1], method, &[]));
    assert!(reg.connect(signal, target, ScriptMemberID(2), &[]));
    assert!(reg.connect(signal, survivors[2], method, &[]));

    let mut snapshot = Vec::new();
    reg.copy_signal_connections(signal, &mut snapshot);
    assert_eq!(reg.disconnect_script(target), 2);

    assert_eq!(snapshot.len(), 5);
    let mut live = Vec::new();
    reg.copy_signal_connections(signal, &mut live);
    assert_eq!(
        live.iter()
            .map(|connection| connection.script_id)
            .collect::<Vec<_>>(),
        survivors
    );
}

#[test]
fn stale_generation_teardown_does_not_remove_reused_node_connections() {
    let signal = SignalID::from_string("reused_node");
    let stale = NodeID::from_parts(7, 1);
    let reused = NodeID::from_parts(7, 2);
    let method = ScriptMemberID::from_string("handle");
    let mut reg = SignalRegistry::new();
    assert!(reg.connect(signal, reused, method, &[]));

    assert_eq!(reg.disconnect_script(stale), 0);

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].script_id, reused);
}
