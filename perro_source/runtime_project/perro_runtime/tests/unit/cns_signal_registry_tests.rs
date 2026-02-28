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
    assert!(reg.connect(signal, id1, f1));
    assert!(!reg.connect(signal, id1, f1));
    assert!(reg.connect(signal, id1, f2));
    assert!(reg.connect(signal, id2, f3));

    let mut out = Vec::new();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(out.len(), 3);

    assert!(reg.disconnect(signal, id1, f2));
    assert!(!reg.disconnect(signal, id1, f2));

    out.clear();
    reg.copy_signal_connections(signal, &mut out);
    assert_eq!(out.len(), 2);
}

#[test]
fn disconnect_script_removes_all_entries() {
    let s1 = SignalID::from_string("s1");
    let s2 = SignalID::from_string("s2");
    let id1 = NodeID::new(10);
    let id2 = NodeID::new(11);
    let f = ScriptMemberID::from_string("h");

    let mut reg = SignalRegistry::new();
    assert!(reg.connect(s1, id1, f));
    assert!(reg.connect(s1, id2, f));
    assert!(reg.connect(s2, id1, f));
    assert_eq!(reg.disconnect_script(id1), 2);

    let mut out = Vec::new();
    reg.copy_signal_connections(s1, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].script_id, id2);

    out.clear();
    reg.copy_signal_connections(s2, &mut out);
    assert!(out.is_empty());
}
