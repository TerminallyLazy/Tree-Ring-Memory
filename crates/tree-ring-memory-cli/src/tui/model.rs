use std::collections::BTreeMap;

use tree_ring_memory_core::models::RINGS;
use tree_ring_memory_core::MemoryEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct RingStats {
    pub ring: String,
    pub total: usize,
    pub event_type_counts: BTreeMap<String, usize>,
    pub sensitive_count: usize,
    pub superseded_count: usize,
    pub average_salience: f64,
    pub average_confidence: f64,
    pub newest_at: Option<String>,
    pub oldest_at: Option<String>,
    pub pulse_level: f64,
    pub warning_level: f64,
}

impl RingStats {
    pub fn empty(ring: &str) -> Self {
        Self {
            ring: ring.to_string(),
            total: 0,
            event_type_counts: BTreeMap::new(),
            sensitive_count: 0,
            superseded_count: 0,
            average_salience: 0.0,
            average_confidence: 0.0,
            newest_at: None,
            oldest_at: None,
            pulse_level: 0.0,
            warning_level: 0.0,
        }
    }

    pub fn top_event_types(&self, max: usize) -> Vec<String> {
        let mut counts: Vec<_> = self.event_type_counts.iter().collect();
        counts.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
        counts
            .into_iter()
            .take(max)
            .map(|(event_type, count)| format!("{event_type}:{count}"))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardStats {
    pub rings: Vec<RingStats>,
    pub total: usize,
    pub sensitive_total: usize,
    pub superseded_total: usize,
    pub last_refresh: Option<String>,
}

impl DashboardStats {
    pub fn empty() -> Self {
        Self {
            rings: RINGS.iter().map(|ring| RingStats::empty(ring)).collect(),
            total: 0,
            sensitive_total: 0,
            superseded_total: 0,
            last_refresh: None,
        }
    }

    pub fn from_memories(memories: &[MemoryEvent], previous: Option<&DashboardStats>) -> Self {
        let mut dashboard = Self::empty();
        dashboard.last_refresh = Some(tree_ring_memory_core::now_iso());

        for memory in memories {
            {
                let Some(stats) = dashboard.ring_mut(&memory.ring) else {
                    continue;
                };
                stats.total += 1;
                *stats
                    .event_type_counts
                    .entry(memory.event_type.clone())
                    .or_insert(0) += 1;
                if memory.sensitivity != "normal" {
                    stats.sensitive_count += 1;
                }
                if memory.superseded_by.is_some() {
                    stats.superseded_count += 1;
                }
                stats.average_salience += memory.salience;
                stats.average_confidence += memory.confidence;
                track_time(&mut stats.newest_at, &memory.created_at, true);
                track_time(&mut stats.oldest_at, &memory.created_at, false);
            }

            if memory.sensitivity != "normal" {
                dashboard.sensitive_total += 1;
            }
            if memory.superseded_by.is_some() {
                dashboard.superseded_total += 1;
            }
            dashboard.total += 1;
        }

        for stats in &mut dashboard.rings {
            if stats.total > 0 {
                stats.average_salience /= stats.total as f64;
                stats.average_confidence /= stats.total as f64;
            }
            stats.warning_level = ring_warning_level(stats);
            if let Some(previous_stats) = previous.and_then(|previous| previous.ring(&stats.ring)) {
                if stats.total != previous_stats.total {
                    stats.pulse_level = 1.0;
                } else {
                    stats.pulse_level = previous_stats.pulse_level * 0.82;
                }
            }
        }

        dashboard
    }

    pub fn ring(&self, ring: &str) -> Option<&RingStats> {
        self.rings.iter().find(|stats| stats.ring == ring)
    }

    pub fn ring_mut(&mut self, ring: &str) -> Option<&mut RingStats> {
        self.rings.iter_mut().find(|stats| stats.ring == ring)
    }

    pub fn pulse_ring(&mut self, ring: &str, level: f64) {
        if let Some(stats) = self.ring_mut(ring) {
            stats.pulse_level = stats.pulse_level.max(level.clamp(0.0, 1.0));
        }
    }

    pub fn decay_pulses(&mut self) {
        for stats in &mut self.rings {
            stats.pulse_level *= 0.88;
            if stats.pulse_level < 0.02 {
                stats.pulse_level = 0.0;
            }
        }
    }
}

fn ring_warning_level(stats: &RingStats) -> f64 {
    if stats.ring == "scar" && stats.total > 0 {
        return 1.0;
    }
    if stats.sensitive_count > 0 && stats.total > 0 {
        return (stats.sensitive_count as f64 / stats.total as f64).clamp(0.25, 1.0);
    }
    0.0
}

fn track_time(target: &mut Option<String>, candidate: &str, newest: bool) {
    match target {
        None => *target = Some(candidate.to_string()),
        Some(current) if newest && candidate > current.as_str() => {
            *current = candidate.to_string();
        }
        Some(current) if !newest && candidate < current.as_str() => {
            *current = candidate.to_string();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(summary: &str, ring: &str, event_type: &str) -> MemoryEvent {
        let mut memory = MemoryEvent::new(summary, event_type).unwrap();
        memory.ring = ring.to_string();
        memory
    }

    #[test]
    fn derives_ring_counts_and_averages() {
        let mut heartwood = event("Durable preference", "heartwood", "user_preference");
        heartwood.salience = 0.8;
        heartwood.confidence = 0.9;
        let mut scar = event("Avoid stale cache", "scar", "warning");
        scar.sensitivity = "private".to_string();

        let stats = DashboardStats::from_memories(&[heartwood, scar], None);

        assert_eq!(stats.total, 2);
        assert_eq!(stats.sensitive_total, 1);
        assert_eq!(stats.ring("heartwood").unwrap().total, 1);
        assert_eq!(
            stats
                .ring("heartwood")
                .unwrap()
                .event_type_counts
                .get("user_preference"),
            Some(&1)
        );
        assert_eq!(stats.ring("scar").unwrap().warning_level, 1.0);
    }

    #[test]
    fn count_deltas_pulse_changed_rings() {
        let first = DashboardStats::from_memories(&[event("Fresh", "cambium", "lesson")], None);
        let second = DashboardStats::from_memories(
            &[
                event("Fresh", "cambium", "lesson"),
                event("Another", "cambium", "decision"),
            ],
            Some(&first),
        );

        assert_eq!(second.ring("cambium").unwrap().pulse_level, 1.0);
    }
}
