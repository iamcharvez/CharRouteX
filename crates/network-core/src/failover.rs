use crate::ConnectivityState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverPolicy {
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    pub cooldown_seconds: u64,
}

impl Default for FailoverPolicy {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_threshold: 3,
            cooldown_seconds: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CandidateHealth {
    pub adapter_id: String,
    pub priority: u32,
    pub state: ConnectivityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailoverDecision {
    Stay,
    Switch {
        from: Option<String>,
        to: String,
        reason: String,
    },
}

#[derive(Debug, Default)]
pub struct FailoverEngine {
    consecutive_good: HashMap<String, u32>,
    consecutive_bad: HashMap<String, u32>,
    active: Option<String>,
    last_switch_seconds: Option<u64>,
}

impl FailoverEngine {
    pub fn set_active_if_unknown(&mut self, adapter_id: Option<String>) {
        if self.active.is_none() {
            self.active = adapter_id;
        }
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn evaluate(
        &mut self,
        now_seconds: u64,
        policy: &FailoverPolicy,
        candidates: &[CandidateHealth],
    ) -> FailoverDecision {
        for candidate in candidates {
            let good = matches!(
                candidate.state,
                ConnectivityState::Healthy | ConnectivityState::Degraded
            );
            if good {
                *self
                    .consecutive_good
                    .entry(candidate.adapter_id.clone())
                    .or_default() += 1;
                self.consecutive_bad.insert(candidate.adapter_id.clone(), 0);
            } else {
                *self
                    .consecutive_bad
                    .entry(candidate.adapter_id.clone())
                    .or_default() += 1;
                self.consecutive_good
                    .insert(candidate.adapter_id.clone(), 0);
            }
        }
        let cooling_down = self
            .last_switch_seconds
            .is_some_and(|last| now_seconds.saturating_sub(last) < policy.cooldown_seconds);
        if cooling_down {
            return FailoverDecision::Stay;
        }
        let active_failed = self.active.as_ref().is_none_or(|id| {
            self.consecutive_bad
                .get(id)
                .is_none_or(|count| *count >= policy.failure_threshold)
        });
        if !active_failed {
            return FailoverDecision::Stay;
        }
        let target = candidates
            .iter()
            .filter(|candidate| {
                self.consecutive_good
                    .get(&candidate.adapter_id)
                    .copied()
                    .unwrap_or(0)
                    >= policy.recovery_threshold
            })
            .min_by_key(|candidate| candidate.priority);
        let Some(target) = target else {
            return FailoverDecision::Stay;
        };
        if self.active.as_deref() == Some(target.adapter_id.as_str()) {
            return FailoverDecision::Stay;
        }
        let from = self.active.replace(target.adapter_id.clone());
        self.last_switch_seconds = Some(now_seconds);
        FailoverDecision::Switch { from, to: target.adapter_id.clone(), reason: "active uplink exceeded the failure threshold and a healthy candidate met the recovery threshold".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(id: &str, priority: u32, state: ConnectivityState) -> CandidateHealth {
        CandidateHealth {
            adapter_id: id.into(),
            priority,
            state,
        }
    }
    #[test]
    fn waits_for_recovery_threshold_before_switching() {
        let policy = FailoverPolicy {
            failure_threshold: 2,
            recovery_threshold: 2,
            cooldown_seconds: 30,
        };
        let mut engine = FailoverEngine::default();
        let candidates = [
            candidate("primary", 1, ConnectivityState::Disconnected),
            candidate("backup", 2, ConnectivityState::Healthy),
        ];
        assert_eq!(
            engine.evaluate(0, &policy, &candidates),
            FailoverDecision::Stay
        );
        assert!(
            matches!(engine.evaluate(1, &policy, &candidates), FailoverDecision::Switch { to, .. } if to == "backup")
        );
    }
    #[test]
    fn cooldown_prevents_flapping() {
        let policy = FailoverPolicy {
            failure_threshold: 1,
            recovery_threshold: 1,
            cooldown_seconds: 30,
        };
        let mut engine = FailoverEngine::default();
        let first = [candidate("a", 1, ConnectivityState::Healthy)];
        assert!(matches!(
            engine.evaluate(10, &policy, &first),
            FailoverDecision::Switch { .. }
        ));
        let second = [
            candidate("a", 1, ConnectivityState::Disconnected),
            candidate("b", 2, ConnectivityState::Healthy),
        ];
        assert_eq!(
            engine.evaluate(20, &policy, &second),
            FailoverDecision::Stay
        );
    }

    #[test]
    fn replaces_a_preferred_route_that_is_not_an_eligible_candidate() {
        let policy = FailoverPolicy {
            failure_threshold: 3,
            recovery_threshold: 1,
            cooldown_seconds: 30,
        };
        let mut engine = FailoverEngine::default();
        engine.set_active_if_unknown(Some("local-only".into()));
        let candidates = [candidate("internet", 10, ConnectivityState::Healthy)];
        assert!(
            matches!(engine.evaluate(10, &policy, &candidates), FailoverDecision::Switch { to, .. } if to == "internet")
        );
    }
}
