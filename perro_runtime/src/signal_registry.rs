use ahash::AHashMap;
use perro_ids::{NodeID, ScriptMemberID, SignalID};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignalConnection {
    pub(crate) script_id: NodeID,
    pub(crate) method_id: ScriptMemberID,
}

pub(crate) struct SignalRegistry {
    by_signal: AHashMap<SignalID, Vec<SignalConnection>>,
}

impl SignalRegistry {
    pub(crate) fn new() -> Self {
        Self {
            by_signal: AHashMap::default(),
        }
    }

    pub(crate) fn connect(
        &mut self,
        signal_id: SignalID,
        script_id: NodeID,
        method_id: ScriptMemberID,
    ) -> bool {
        let connections = self
            .by_signal
            .entry(signal_id)
            .or_insert_with(|| Vec::with_capacity(1));
        if connections
            .iter()
            .any(|c| c.script_id == script_id && c.method_id == method_id)
        {
            return false;
        }
        connections.push(SignalConnection {
            script_id,
            method_id,
        });
        true
    }

    pub(crate) fn disconnect(
        &mut self,
        signal_id: SignalID,
        script_id: NodeID,
        method_id: ScriptMemberID,
    ) -> bool {
        let Some(connections) = self.by_signal.get_mut(&signal_id) else {
            return false;
        };
        let Some(i) = connections
            .iter()
            .position(|c| c.script_id == script_id && c.method_id == method_id)
        else {
            return false;
        };
        connections.swap_remove(i);
        if connections.is_empty() {
            self.by_signal.remove(&signal_id);
        }
        true
    }

    pub(crate) fn copy_signal_connections(
        &self,
        signal_id: SignalID,
        out: &mut Vec<SignalConnection>,
    ) {
        let Some(connections) = self.by_signal.get(&signal_id) else {
            return;
        };
        out.extend_from_slice(connections);
    }

    #[inline]
    pub(crate) fn single_signal_connection(&self, signal_id: SignalID) -> Option<SignalConnection> {
        let connections = self.by_signal.get(&signal_id)?;
        (connections.len() == 1).then_some(connections[0])
    }

    #[inline]
    pub(crate) fn signal_connection_count(&self, signal_id: SignalID) -> usize {
        self.by_signal.get(&signal_id).map_or(0, Vec::len)
    }

    pub(crate) fn disconnect_script(&mut self, script_id: NodeID) -> usize {
        let mut removed = 0usize;
        let mut empty_signals = Vec::new();
        for (signal_id, connections) in self.by_signal.iter_mut() {
            let before = connections.len();
            connections.retain(|c| c.script_id != script_id);
            removed += before - connections.len();
            if connections.is_empty() {
                empty_signals.push(*signal_id);
            }
        }
        for signal_id in empty_signals {
            self.by_signal.remove(&signal_id);
        }
        removed
    }
}

impl Default for SignalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
