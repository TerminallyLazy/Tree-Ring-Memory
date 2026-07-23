use std::{collections::HashSet, fmt};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tree_ring_memory_core::models::{
    now_iso, sqlite_error, MemoryEvent, TreeRingError, TreeRingResult,
};
use tree_ring_memory_core::SensitivityGuard;
use uuid::Uuid;

use crate::sqlite_error_from_rusqlite;

const POLICY_SINGLETON_ID: i64 = 1;
const MAX_CONTEXT_VALUE_LENGTH: usize = 256;
const MAX_CAPABILITY_LENGTH: usize = 4096;
const CAPABILITY_HASH_DOMAIN: &[u8] = b"tree-ring-coordinator-capability-v1";
const AUDIT_TARGET_HASH_DOMAIN: &[u8] = b"tree-ring-authorization-audit-target-v1";

#[derive(Clone, PartialEq, Eq)]
pub struct WriteContext {
    pub(crate) actor_profile: Option<String>,
    pub(crate) capability_hash: Option<[u8; 32]>,
    pub(crate) origin: String,
}

impl WriteContext {
    pub fn new(
        actor_profile: Option<String>,
        capability: Option<&str>,
        origin: impl Into<String>,
    ) -> TreeRingResult<Self> {
        if let Some(actor_profile) = actor_profile.as_deref() {
            validate_context_value("actor_profile", actor_profile)?;
        }
        let origin = origin.into();
        validate_context_value("audit origin", &origin)?;
        let capability_hash = capability
            .map(|capability| {
                if capability.is_empty() {
                    return Err(TreeRingError::Validation(
                        "coordinator capability cannot be empty".to_string(),
                    ));
                }
                if capability.len() > MAX_CAPABILITY_LENGTH {
                    return Err(TreeRingError::Validation(format!(
                        "coordinator capability must be at most {MAX_CAPABILITY_LENGTH} bytes"
                    )));
                }
                Ok(coordinator_capability_hash(capability))
            })
            .transpose()?;
        Ok(Self {
            actor_profile,
            capability_hash,
            origin,
        })
    }

    pub(crate) fn anonymous() -> Self {
        Self {
            actor_profile: None,
            capability_hash: None,
            origin: "anonymous".to_string(),
        }
    }
}

impl fmt::Debug for WriteContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriteContext")
            .field("actor_profile", &self.actor_profile)
            .field("has_capability", &self.capability_hash.is_some())
            .field("origin", &self.origin)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    Open,
    Coordinated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyStatus {
    pub mode: PolicyMode,
    pub coordinator_label: Option<String>,
    pub enabled_at: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyGrant {
    pub capability: String,
    pub status: PolicyStatus,
}

impl fmt::Debug for PolicyGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyGrant")
            .field("capability", &"[REDACTED]")
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationAuditEvent {
    pub id: i64,
    pub created_at: String,
    pub action: String,
    pub decision: String,
    pub reason: String,
    pub actor_profile: Option<String>,
    pub origin: String,
    pub target_memory_id: Option<String>,
}

#[derive(Debug)]
pub(crate) enum AuthorizationOutcome {
    Allowed,
    Denied(TreeRingError),
}

pub(crate) fn create_policy_schema(connection: &Connection) -> TreeRingResult<()> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS store_policy (
              id INTEGER PRIMARY KEY NOT NULL CHECK(id = 1),
              mode TEXT NOT NULL CHECK(mode IN ('open', 'coordinated')),
              capability_hash BLOB
                CHECK(capability_hash IS NULL OR length(capability_hash) = 32),
              coordinator_label TEXT,
              enabled_at TEXT,
              updated_at TEXT NOT NULL,
              CHECK(
                (mode = 'open' AND capability_hash IS NULL)
                OR (mode = 'coordinated' AND capability_hash IS NOT NULL)
              )
            );
            CREATE TABLE IF NOT EXISTS authorization_audit (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              created_at TEXT NOT NULL,
              action TEXT NOT NULL,
              decision TEXT NOT NULL CHECK(decision IN ('allowed', 'denied')),
              reason TEXT NOT NULL,
              actor_profile TEXT,
              origin TEXT NOT NULL,
              target_memory_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_authorization_audit_created_at
              ON authorization_audit(created_at DESC, id DESC);
            "#,
        )
        .map_err(sqlite_error_from_rusqlite)?;
    let now = now_iso();
    connection
        .execute(
            r#"
            INSERT OR IGNORE INTO store_policy (
              id, mode, capability_hash, coordinator_label, enabled_at, updated_at
            ) VALUES (?, 'open', NULL, NULL, NULL, ?)
            "#,
            params![POLICY_SINGLETON_ID, now],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

pub(crate) fn policy_status_on_connection(connection: &Connection) -> TreeRingResult<PolicyStatus> {
    connection
        .query_row(
            r#"
            SELECT mode, coordinator_label, enabled_at, updated_at
            FROM store_policy
            WHERE id = ?
            "#,
            params![POLICY_SINGLETON_ID],
            |row| {
                let mode: String = row.get(0)?;
                let mode = match mode.as_str() {
                    "open" => PolicyMode::Open,
                    "coordinated" => PolicyMode::Coordinated,
                    _ => {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            format!("unknown store policy mode {mode:?}").into(),
                        ));
                    }
                };
                Ok(PolicyStatus {
                    mode,
                    coordinator_label: row.get(1)?,
                    enabled_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .map_err(sqlite_error_from_rusqlite)
}

pub(crate) fn policy_audit_on_connection(
    connection: &Connection,
    limit: usize,
) -> TreeRingResult<Vec<AuthorizationAuditEvent>> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, created_at, action, decision, reason, actor_profile, origin,
                   target_memory_id
            FROM authorization_audit
            ORDER BY id DESC
            LIMIT ?
            "#,
        )
        .map_err(sqlite_error_from_rusqlite)?;
    let rows = statement
        .query_map(params![limit as i64], |row| {
            Ok(AuthorizationAuditEvent {
                id: row.get(0)?,
                created_at: row.get(1)?,
                action: row.get(2)?,
                decision: row.get(3)?,
                reason: row.get(4)?,
                actor_profile: row.get(5)?,
                origin: row.get(6)?,
                target_memory_id: row.get(7)?,
            })
        })
        .map_err(sqlite_error_from_rusqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error_from_rusqlite)
}

pub(crate) fn enable_policy(
    connection: &Connection,
    context: &WriteContext,
    label: Option<&str>,
) -> TreeRingResult<Result<PolicyGrant, TreeRingError>> {
    validate_optional_label(label)?;
    let current = policy_row(connection)?;
    if current.mode == PolicyMode::Coordinated {
        insert_audit(
            connection,
            context,
            "policy_enable",
            "denied",
            "coordinated_policy_already_enabled",
            None,
        )?;
        return Ok(Err(TreeRingError::AuthorizationDenied(
            "coordinated policy is already enabled; rotate the coordinator capability instead"
                .to_string(),
        )));
    }

    let capability = generated_coordinator_capability();
    let capability_hash = coordinator_capability_hash(&capability);
    let now = now_iso();
    connection
        .execute(
            r#"
            UPDATE store_policy
            SET mode = 'coordinated',
                capability_hash = ?,
                coordinator_label = ?,
                enabled_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
            params![
                capability_hash.as_slice(),
                label,
                &now,
                &now,
                POLICY_SINGLETON_ID
            ],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    insert_audit(
        connection,
        context,
        "policy_enable",
        "allowed",
        "coordinated_policy_enabled",
        None,
    )?;
    Ok(Ok(PolicyGrant {
        capability,
        status: policy_status_on_connection(connection)?,
    }))
}

pub(crate) fn rotate_policy_capability(
    connection: &Connection,
    context: &WriteContext,
    label: Option<&str>,
) -> TreeRingResult<Result<PolicyGrant, TreeRingError>> {
    validate_optional_label(label)?;
    let policy = policy_row(connection)?;
    if policy.mode == PolicyMode::Open {
        insert_audit(
            connection,
            context,
            "policy_rotate",
            "denied",
            "coordinated_policy_not_enabled",
            None,
        )?;
        return Ok(Err(TreeRingError::AuthorizationDenied(
            "coordinated policy must be enabled before rotating its capability".to_string(),
        )));
    }
    if let AuthorizationOutcome::Denied(error) =
        authorize_with_policy(connection, context, "policy_rotate", None, &policy)?
    {
        return Ok(Err(error));
    }

    let capability = generated_coordinator_capability();
    let capability_hash = coordinator_capability_hash(&capability);
    let now = now_iso();
    connection
        .execute(
            r#"
            UPDATE store_policy
            SET capability_hash = ?,
                coordinator_label = COALESCE(?, coordinator_label),
                updated_at = ?
            WHERE id = ? AND mode = 'coordinated'
            "#,
            params![capability_hash.as_slice(), label, &now, POLICY_SINGLETON_ID],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(Ok(PolicyGrant {
        capability,
        status: policy_status_on_connection(connection)?,
    }))
}

pub(crate) fn disable_policy(
    connection: &Connection,
    context: &WriteContext,
) -> TreeRingResult<Result<PolicyStatus, TreeRingError>> {
    if let AuthorizationOutcome::Denied(error) =
        authorize_coordinator_action(connection, context, "policy_disable", None)?
    {
        return Ok(Err(error));
    }

    let now = now_iso();
    connection
        .execute(
            r#"
            UPDATE store_policy
            SET mode = 'open',
                capability_hash = NULL,
                coordinator_label = NULL,
                enabled_at = NULL,
                updated_at = ?
            WHERE id = ?
            "#,
            params![now, POLICY_SINGLETON_ID],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(Ok(policy_status_on_connection(connection)?))
}

pub(crate) fn authorize_event_creates(
    connection: &Connection,
    context: &WriteContext,
    action: &str,
    events: &[&MemoryEvent],
) -> TreeRingResult<AuthorizationOutcome> {
    let policy = policy_row(connection)?;
    if policy.mode == PolicyMode::Open || events.is_empty() {
        return Ok(AuthorizationOutcome::Allowed);
    }

    let mut seen_memory_ids = HashSet::new();
    let protected_target = events
        .iter()
        .find_map(|event| {
            let existing = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM memories WHERE id = ?)",
                    params![&event.id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(sqlite_error_from_rusqlite);
            let repeated_in_request = !seen_memory_ids.insert(event.id.as_str());
            match existing {
                Ok(true) => Some(Ok(event.id.as_str())),
                Ok(false)
                    if !repeated_in_request && unprivileged_create_allowed(context, event) =>
                {
                    None
                }
                Ok(false) => Some(Ok(event.id.as_str())),
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?;
    if protected_target.is_none() {
        return Ok(AuthorizationOutcome::Allowed);
    }
    authorize_with_policy(connection, context, action, protected_target, &policy)
}

pub(crate) fn authorize_coordinator_action(
    connection: &Connection,
    context: &WriteContext,
    action: &str,
    target_memory_id: Option<&str>,
) -> TreeRingResult<AuthorizationOutcome> {
    let policy = policy_row(connection)?;
    if policy.mode == PolicyMode::Open {
        return Ok(AuthorizationOutcome::Allowed);
    }
    authorize_with_policy(connection, context, action, target_memory_id, &policy)
}

fn authorize_with_policy(
    connection: &Connection,
    context: &WriteContext,
    action: &str,
    target_memory_id: Option<&str>,
    policy: &PolicyRow,
) -> TreeRingResult<AuthorizationOutcome> {
    let supplied = context.capability_hash.as_ref();
    let valid = supplied
        .zip(policy.capability_hash.as_ref())
        .is_some_and(|(supplied, expected)| constant_time_eq(supplied, expected));
    if valid {
        insert_audit(
            connection,
            context,
            action,
            "allowed",
            "valid_coordinator_capability",
            target_memory_id,
        )?;
        return Ok(AuthorizationOutcome::Allowed);
    }

    let reason = if supplied.is_some() {
        "invalid_coordinator_capability"
    } else {
        "missing_coordinator_capability"
    };
    insert_audit(
        connection,
        context,
        action,
        "denied",
        reason,
        target_memory_id,
    )?;
    Ok(AuthorizationOutcome::Denied(
        TreeRingError::AuthorizationDenied(
            "coordinator capability required by coordinated store policy".to_string(),
        ),
    ))
}

fn insert_audit(
    connection: &Connection,
    context: &WriteContext,
    action: &str,
    decision: &str,
    reason: &str,
    target_memory_id: Option<&str>,
) -> TreeRingResult<()> {
    let target_memory_id = target_memory_id.map(normalize_audit_target);
    connection
        .execute(
            r#"
            INSERT INTO authorization_audit (
              created_at, action, decision, reason, actor_profile, origin,
              target_memory_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                now_iso(),
                action,
                decision,
                reason,
                context.actor_profile.as_deref(),
                &context.origin,
                target_memory_id.as_deref(),
            ],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

fn normalize_audit_target(value: &str) -> String {
    let display_safe = !value.is_empty()
        && value.len() <= MAX_CONTEXT_VALUE_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
        && SensitivityGuard::default().check_or_raise(value).is_ok();
    if display_safe {
        return value.to_string();
    }

    let mut hasher = Sha256::new();
    hasher.update(AUDIT_TARGET_HASH_DOMAIN);
    hasher.update([0]);
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[derive(Debug)]
struct PolicyRow {
    mode: PolicyMode,
    capability_hash: Option<[u8; 32]>,
}

fn policy_row(connection: &Connection) -> TreeRingResult<PolicyRow> {
    let row = connection
        .query_row(
            "SELECT mode, capability_hash FROM store_policy WHERE id = ?",
            params![POLICY_SINGLETON_ID],
            |row| {
                let mode: String = row.get(0)?;
                let capability_hash: Option<Vec<u8>> = row.get(1)?;
                Ok((mode, capability_hash))
            },
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .ok_or_else(|| sqlite_error("store policy singleton row is missing"))?;
    let mode = match row.0.as_str() {
        "open" => PolicyMode::Open,
        "coordinated" => PolicyMode::Coordinated,
        other => return Err(sqlite_error(format!("unknown store policy mode {other:?}"))),
    };
    let capability_hash = row
        .1
        .map(|hash| {
            hash.try_into()
                .map_err(|_| sqlite_error("store policy capability hash must be 32 bytes"))
        })
        .transpose()?;
    Ok(PolicyRow {
        mode,
        capability_hash,
    })
}

fn unprivileged_create_allowed(context: &WriteContext, event: &MemoryEvent) -> bool {
    event.scope == "agent"
        && event.ring != "heartwood"
        && context.actor_profile.as_deref() == event.agent_profile.as_deref()
        && context.actor_profile.is_some()
}

fn validate_context_value(field: &str, value: &str) -> TreeRingResult<()> {
    if value.trim().is_empty() {
        return Err(TreeRingError::Validation(format!(
            "{field} cannot be blank"
        )));
    }
    if value.chars().count() > MAX_CONTEXT_VALUE_LENGTH {
        return Err(TreeRingError::Validation(format!(
            "{field} must be at most {MAX_CONTEXT_VALUE_LENGTH} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(TreeRingError::Validation(format!(
            "{field} cannot contain control characters"
        )));
    }
    SensitivityGuard::default().check_or_raise(value)?;
    Ok(())
}

fn validate_optional_label(label: Option<&str>) -> TreeRingResult<()> {
    if let Some(label) = label {
        validate_context_value("coordinator label", label)?;
    }
    Ok(())
}

fn generated_coordinator_capability() -> String {
    format!(
        "trcap_v1_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn coordinator_capability_hash(capability: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITY_HASH_DOMAIN);
    hasher.update([0]);
    hasher.update((capability.len() as u64).to_be_bytes());
    hasher.update(capability.as_bytes());
    hasher.finalize().into()
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_context_debug_never_contains_capability_or_hash() {
        let capability = "do-not-log-this-capability";
        let context =
            WriteContext::new(Some("agent-a".to_string()), Some(capability), "unit-test").unwrap();
        let rendered = format!("{context:?}");
        assert!(!rendered.contains(capability));
        assert!(!rendered.contains(&format!("{:?}", context.capability_hash.unwrap())));
        assert!(rendered.contains("has_capability: true"));
    }

    #[test]
    fn policy_grant_debug_redacts_one_time_capability() {
        let grant = PolicyGrant {
            capability: "do-not-log-this-capability".to_string(),
            status: PolicyStatus {
                mode: PolicyMode::Coordinated,
                coordinator_label: None,
                enabled_at: Some("2026-01-01T00:00:00Z".to_string()),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        };
        let rendered = format!("{grant:?}");
        assert!(!rendered.contains(&grant.capability));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn write_context_limits_are_measured_in_unicode_characters() {
        let boundary = "é".repeat(MAX_CONTEXT_VALUE_LENGTH);
        WriteContext::new(Some(boundary), None, "unit-test").unwrap();

        let oversized = "é".repeat(MAX_CONTEXT_VALUE_LENGTH + 1);
        let error = WriteContext::new(Some(oversized), None, "unit-test").unwrap_err();
        assert!(matches!(
            error,
            TreeRingError::Validation(message)
                if message.contains("256 characters")
        ));
    }

    #[test]
    fn audit_targets_preserve_safe_ids_and_hash_unsafe_or_secret_values() {
        assert_eq!(normalize_audit_target("mem_safe-123"), "mem_safe-123");

        let unsafe_values = [
            "bad\nid\u{1b}[31m".to_string(),
            format!("trcap_v1_{}", "a".repeat(64)),
            "x".repeat(MAX_CONTEXT_VALUE_LENGTH + 1),
        ];
        for unsafe_value in &unsafe_values {
            let normalized = normalize_audit_target(unsafe_value);
            assert!(normalized.starts_with("sha256:"));
            assert_eq!(normalized.len(), 71);
            assert!(!normalized.contains(unsafe_value));
            assert!(!normalized.chars().any(char::is_control));
        }
    }
}
