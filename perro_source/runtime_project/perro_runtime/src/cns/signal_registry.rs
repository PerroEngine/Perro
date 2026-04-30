use ahash::AHashMap;
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SignalConnection {
    pub(crate) script_id: NodeID,
    pub(crate) method: ScriptMemberID,
    pub(crate) params: Vec<Variant>,
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
        signal: SignalID,
        script_id: NodeID,
        method: ScriptMemberID,
        params: &[Variant],
    ) -> bool {
        let connections = self
            .by_signal
            .entry(signal)
            .or_insert_with(|| Vec::with_capacity(1));
        if connections
            .iter()
            .any(|c| c.script_id == script_id && c.method == method)
        {
            return false;
        }
        connections.push(SignalConnection {
            script_id,
            method,
            params: params.to_vec(),
        });
        true
    }

    pub(crate) fn disconnect(
        &mut self,
        signal: SignalID,
        script_id: NodeID,
        method: ScriptMemberID,
    ) -> bool {
        let Some(connections) = self.by_signal.get_mut(&signal) else {
            return false;
        };
        let Some(i) = connections
            .iter()
            .position(|c| c.script_id == script_id && c.method == method)
        else {
            return false;
        };
        connections.swap_remove(i);
        if connections.is_empty() {
            self.by_signal.remove(&signal);
        }
        true
    }

    pub(crate) fn copy_signal_connections(
        &self,
        signal: SignalID,
        out: &mut Vec<SignalConnection>,
    ) {
        let Some(connections) = self.by_signal.get(&signal) else {
            return;
        };
        out.extend(connections.iter().cloned());
    }

    #[inline]
    pub(crate) fn single_signal_connection(&self, signal: SignalID) -> Option<SignalConnection> {
        let connections = self.by_signal.get(&signal)?;
        (connections.len() == 1).then(|| connections[0].clone())
    }

    pub(crate) fn disconnect_script(&mut self, script_id: NodeID) -> usize {
        let mut removed = 0usize;
        let mut empty_signals = Vec::new();
        for (signal, connections) in self.by_signal.iter_mut() {
            let before = connections.len();
            connections.retain(|c| c.script_id != script_id);
            removed += before - connections.len();
            if connections.is_empty() {
                empty_signals.push(*signal);
            }
        }
        for signal in empty_signals {
            self.by_signal.remove(&signal);
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
#[path = "../../tests/unit/cns_signal_registry_tests.rs"]
mod tests;
