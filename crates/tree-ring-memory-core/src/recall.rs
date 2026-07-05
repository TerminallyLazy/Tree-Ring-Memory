use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};

use crate::models::MemoryEvent;

const FAILURE_TERMS: &[&str] = &[
    "error",
    "failure",
    "regression",
    "bug",
    "rejected",
    "rollback",
    "stale",
    "conflict",
    "security",
    "privacy",
    "mistake",
];
const HEARTWOOD_TERMS: &[&str] = &["preference", "rule", "constraint", "decision", "durable"];
const SEED_TERMS: &[&str] = &[
    "planning",
    "roadmap",
    "future",
    "alternative",
    "experiment",
    "explore",
];

#[derive(Debug, Clone, PartialEq)]
pub struct RecallRanking {
    pub factors: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecallScore {
    pub score: f64,
    pub ranking: RecallRanking,
}

#[derive(Debug, Default)]
pub struct RecallScorer;

impl RecallScorer {
    pub fn score(event: &MemoryEvent, query: &str) -> RecallScore {
        let textual = textual_match(event, query);
        let recency = recency_score(&event.created_at);
        let authority = source_authority(event);
        let ring_boost = ring_boost(event, query);

        let mut factors = BTreeMap::new();
        factors.insert("textual_match".to_string(), textual);
        factors.insert("salience".to_string(), event.salience);
        factors.insert("confidence".to_string(), event.confidence);
        factors.insert("recency".to_string(), recency);
        factors.insert("source_authority".to_string(), authority);
        factors.insert("ring_boost".to_string(), ring_boost);

        let score = 0.25 * textual
            + 0.25 * event.salience
            + 0.25 * event.confidence
            + 0.20 * recency
            + 0.05 * authority
            + ring_boost;

        RecallScore {
            score,
            ranking: RecallRanking { factors },
        }
    }
}

pub fn terms(text: &str) -> Vec<String> {
    TERM_RE
        .find_iter(text)
        .map(|term| term.as_str().to_ascii_lowercase())
        .collect()
}

pub fn search_queries(query: &str) -> Vec<String> {
    let query_terms = terms(query);
    let mut queries = vec![query.to_string()];
    let intent_terms: HashSet<&str> = FAILURE_TERMS
        .iter()
        .chain(HEARTWOOD_TERMS)
        .chain(SEED_TERMS)
        .copied()
        .collect();

    for (index, term) in query_terms.iter().enumerate() {
        if intent_terms.contains(term.as_str()) {
            let remaining: Vec<&str> = query_terms
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    if idx == index {
                        None
                    } else {
                        Some(item.as_str())
                    }
                })
                .collect();
            if !remaining.is_empty() {
                queries.push(remaining.join(" "));
            }
        }
    }
    queries
}

fn textual_match(event: &MemoryEvent, query: &str) -> f64 {
    let query_terms = terms(query);
    if query_terms.is_empty() {
        return 0.1;
    }
    let text = format!(
        "{} {} {}",
        event.summary,
        event.details,
        event.tags.join(" ")
    )
    .to_ascii_lowercase();
    let matches = query_terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count();
    matches as f64 / query_terms.len() as f64
}

fn recency_score(created_at: &str) -> f64 {
    let created_at = parse_timestamp(created_at).unwrap_or_else(Utc::now);
    let age_seconds = (Utc::now() - created_at).num_seconds().max(0) as f64;
    (-age_seconds / 86_400.0 / 30.0).exp()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Some(value.with_timezone(&Utc));
    }
    if let Ok(value) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(value.and_utc());
    }
    None
}

fn source_authority(event: &MemoryEvent) -> f64 {
    match event.source.source_type.as_str() {
        "user" => 1.0,
        "contract" => 0.9,
        "eval" => 0.8,
        "file" => 0.7,
        "tool" => 0.6,
        "summary" => 0.5,
        "manual" => 0.4,
        _ => 0.3,
    }
}

fn ring_boost(event: &MemoryEvent, query: &str) -> f64 {
    let query_terms: HashSet<String> = terms(query).into_iter().collect();
    if event.ring == "scar" && FAILURE_TERMS.iter().any(|term| query_terms.contains(*term)) {
        return 0.2;
    }
    if event.ring == "heartwood"
        && HEARTWOOD_TERMS
            .iter()
            .any(|term| query_terms.contains(*term))
    {
        return 0.15;
    }
    if event.ring == "seed" && SEED_TERMS.iter().any(|term| query_terms.contains(*term)) {
        return 0.12;
    }
    0.0
}

static TERM_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\w+").unwrap());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MemoryEvent;

    #[test]
    fn scar_boosts_failure_query() {
        let mut event = MemoryEvent::new("Avoid stale frontend cache.", "warning").unwrap();
        event.ring = "scar".to_string();

        let result = RecallScorer::score(&event, "failure stale cache");

        assert_eq!(result.ranking.factors["ring_boost"], 0.2);
        assert!(result.score > 0.0);
    }

    #[test]
    fn search_queries_drop_ring_intent_terms() {
        assert_eq!(
            search_queries("failure stale cache"),
            vec!["failure stale cache", "stale cache", "failure cache"]
        );
    }
}
