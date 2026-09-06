use ahash::AHashMap;
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;
use smallvec::{SmallVec, smallvec};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SignalConnection {
    pub(crate) script_id: NodeID,
    pub(crate) method: ScriptMemberID,
    params: Option<Rc<[Variant]>>,
}

impl SignalConnection {
    fn new(script_id: NodeID, method: ScriptMemberID, params: &[Variant]) -> Self {
        Self {
            script_id,
            method,
            params: (!params.is_empty()).then(|| Rc::from(params)),
        }
    }

    #[inline]
    pub(crate) fn params(&self) -> &[Variant] {
        self.params.as_deref().unwrap_or(&[])
    }
}

enum SignalBucket {
    Single(SignalConnection),
    Multiple(Rc<SmallVec<[SignalConnection; 4]>>),
}

impl SignalBucket {
    #[cfg(any(test, feature = "bench", feature = "profile"))]
    fn connection_count(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multiple(connections) => connections.len(),
        }
    }

    fn contains(&self, script_id: NodeID, method: ScriptMemberID) -> bool {
        match self {
            Self::Single(connection) => {
                connection.script_id == script_id && connection.method == method
            }
            Self::Multiple(connections) => connections
                .iter()
                .any(|connection| connection.script_id == script_id && connection.method == method),
        }
    }

    fn push(&mut self, connection: SignalConnection) {
        match self {
            Self::Single(existing) => {
                *self = Self::Multiple(Rc::new(smallvec![existing.clone(), connection]));
            }
            Self::Multiple(connections) => Rc::make_mut(connections).push(connection),
        }
    }

    #[cfg(test)]
    fn copy_connections(&self, out: &mut Vec<SignalConnection>) {
        match self {
            Self::Single(connection) => out.push(connection.clone()),
            Self::Multiple(connections) => out.extend(connections.iter().cloned()),
        }
    }
}

pub(crate) enum SignalConnectionsSnapshot {
    Single(SignalConnection),
    Multiple(Rc<SmallVec<[SignalConnection; 4]>>),
}

pub(crate) struct SignalRegistry {
    by_signal: AHashMap<SignalID, SignalBucket>,
    #[cfg(any(test, feature = "bench", feature = "profile"))]
    disconnect_script_bucket_scans: usize,
    #[cfg(any(test, feature = "bench", feature = "profile"))]
    disconnect_script_connection_checks: usize,
}

impl SignalRegistry {
    pub(crate) fn new() -> Self {
        Self {
            by_signal: AHashMap::default(),
            #[cfg(any(test, feature = "bench", feature = "profile"))]
            disconnect_script_bucket_scans: 0,
            #[cfg(any(test, feature = "bench", feature = "profile"))]
            disconnect_script_connection_checks: 0,
        }
    }

    pub(crate) fn connect(
        &mut self,
        signal: SignalID,
        script_id: NodeID,
        method: ScriptMemberID,
        params: &[Variant],
    ) -> bool {
        match self.by_signal.entry(signal) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if entry.get().contains(script_id, method) {
                    return false;
                }
                entry
                    .get_mut()
                    .push(SignalConnection::new(script_id, method, params));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SignalBucket::Single(SignalConnection::new(
                    script_id, method, params,
                )));
            }
        }
        true
    }

    pub(crate) fn disconnect(
        &mut self,
        signal: SignalID,
        script_id: NodeID,
        method: ScriptMemberID,
    ) -> bool {
        let remove_bucket = {
            let Some(bucket) = self.by_signal.get_mut(&signal) else {
                return false;
            };
            match bucket {
                SignalBucket::Single(connection) => {
                    if connection.script_id != script_id || connection.method != method {
                        return false;
                    }
                    true
                }
                SignalBucket::Multiple(connections) => {
                    let Some(i) = connections
                        .iter()
                        .position(|c| c.script_id == script_id && c.method == method)
                    else {
                        return false;
                    };
                    let connections = Rc::make_mut(connections);
                    connections.swap_remove(i);
                    connections.is_empty()
                }
            }
        };
        if remove_bucket {
            self.by_signal.remove(&signal);
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn copy_signal_connections(
        &self,
        signal: SignalID,
        out: &mut Vec<SignalConnection>,
    ) {
        let Some(bucket) = self.by_signal.get(&signal) else {
            return;
        };
        bucket.copy_connections(out);
    }

    pub(crate) fn signal_connections_snapshot(
        &self,
        signal: SignalID,
    ) -> Option<SignalConnectionsSnapshot> {
        match self.by_signal.get(&signal)? {
            SignalBucket::Single(connection) => {
                Some(SignalConnectionsSnapshot::Single(connection.clone()))
            }
            SignalBucket::Multiple(connections) => {
                Some(SignalConnectionsSnapshot::Multiple(Rc::clone(connections)))
            }
        }
    }

    pub(crate) fn disconnect_script(&mut self, script_id: NodeID) -> usize {
        let mut removed = 0usize;
        #[cfg(any(test, feature = "bench", feature = "profile"))]
        let mut bucket_scans = 0usize;
        #[cfg(any(test, feature = "bench", feature = "profile"))]
        let mut connection_checks = 0usize;
        self.by_signal.retain(|_, bucket| {
            #[cfg(any(test, feature = "bench", feature = "profile"))]
            {
                bucket_scans += 1;
                connection_checks += bucket.connection_count();
            }
            match bucket {
                SignalBucket::Single(connection) => {
                    if connection.script_id == script_id {
                        removed += 1;
                        false
                    } else {
                        true
                    }
                }
                SignalBucket::Multiple(connections) => {
                    if Rc::strong_count(connections) > 1
                        && !connections
                            .iter()
                            .any(|connection| connection.script_id == script_id)
                    {
                        return true;
                    }
                    let connections = Rc::make_mut(connections);
                    let before = connections.len();
                    connections.retain(|connection| connection.script_id != script_id);
                    removed += before - connections.len();
                    !connections.is_empty()
                }
            }
        });
        #[cfg(any(test, feature = "bench", feature = "profile"))]
        {
            self.disconnect_script_bucket_scans += bucket_scans;
            self.disconnect_script_connection_checks += connection_checks;
        }
        removed
    }

    #[cfg(any(test, feature = "bench", feature = "profile"))]
    pub(crate) fn reset_disconnect_script_counters(&mut self) {
        self.disconnect_script_bucket_scans = 0;
        self.disconnect_script_connection_checks = 0;
    }

    #[cfg(any(test, feature = "bench", feature = "profile"))]
    pub(crate) fn disconnect_script_counters(&self) -> (usize, usize) {
        (
            self.disconnect_script_bucket_scans,
            self.disconnect_script_connection_checks,
        )
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
