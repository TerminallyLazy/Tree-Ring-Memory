import json

from tree_ring_memory.api import PythonTreeRingMemory
from tree_ring_memory.native_backend import NativeTreeRingMemory


def test_python_reference_consolidate_dry_run_writes_nothing(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    first = memory.remember(
        summary="Keep SQLite consolidation deterministic.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )
    second = memory.remember(
        summary="Prefer source-linked summary rows.",
        event_type="lesson",
        project="alpha",
        salience=0.8,
    )

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-test",
        project="alpha",
        dry_run=True,
    )

    assert report["status"] == "dry_run"
    assert report["candidate_count"] == 2
    assert report["source_memory_ids"] == sorted([first.id, second.id])
    assert report["output_memory_ids"]
    assert [event.event_type for event in memory.store.list_all()] == ["lesson", "decision"]


def test_python_reference_consolidate_empty_writes_no_records(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-empty",
        project="alpha",
    )

    records = memory.store.connection.execute("SELECT count(*) FROM consolidations").fetchone()[0]
    rows = memory.store.connection.execute("SELECT count(*) FROM memories").fetchone()[0]
    assert report["status"] == "empty"
    assert report["candidate_count"] == 0
    assert rows == 0
    assert records == 0


def test_python_reference_consolidate_is_idempotent_unless_forced(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    source = memory.remember(
        summary="Consolidation idempotency should be stable.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )

    created = memory.consolidate(
        period_type="manual",
        period_key="manual-idempotent",
        project="alpha",
    )
    unchanged = memory.consolidate(
        period_type="manual",
        period_key="manual-idempotent",
        project="alpha",
    )
    forced = memory.consolidate(
        period_type="manual",
        period_key="manual-idempotent",
        project="alpha",
        force=True,
    )

    assert created["status"] == "created"
    assert created["source_memory_ids"] == [source.id]
    assert unchanged["status"] == "unchanged"
    assert unchanged["output_memory_ids"] == created["output_memory_ids"]
    assert unchanged["outputs"] == []
    assert forced["status"] == "created"
    assert forced["output_memory_ids"] != created["output_memory_ids"]
    old_summary = memory.store.get(created["output_memory_ids"][0])
    assert old_summary is not None
    assert old_summary.superseded_by == forced["output_memory_ids"][0]


def test_python_reference_force_maps_multiple_outputs_to_matching_replacements(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    decision = memory.remember(
        summary="Decision source.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )
    lesson = memory.remember(
        summary="Lesson source.",
        event_type="lesson",
        project="alpha",
        salience=0.8,
    )
    first = memory.consolidate(
        period_type="manual",
        period_key="manual-multi-output",
        project="alpha",
    )

    def output_id_for_source(report, source_id):
        for output in report["outputs"]:
            links = output["memory"]["links"]
            if any(link["type"] == "memory" and link["target"] == source_id for link in links):
                return output["memory"]["id"]
        raise AssertionError(f"missing output for {source_id}")

    old_decision_output_id = output_id_for_source(first, decision.id)
    old_lesson_output_id = output_id_for_source(first, lesson.id)
    forced = memory.consolidate(
        period_type="manual",
        period_key="manual-multi-output",
        project="alpha",
        force=True,
    )

    new_decision_output_id = output_id_for_source(forced, decision.id)
    new_lesson_output_id = output_id_for_source(forced, lesson.id)
    assert memory.store.get(old_decision_output_id).superseded_by == new_decision_output_id
    assert memory.store.get(old_lesson_output_id).superseded_by == new_lesson_output_id
    assert new_decision_output_id != new_lesson_output_id


def test_python_reference_force_supersedes_prior_summary_when_source_set_changes(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    first_source = memory.remember(
        summary="Initial consolidation source.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )
    created = memory.consolidate(
        period_type="manual",
        period_key="manual-changing-source-set",
        project="alpha",
    )
    second_source = memory.remember(
        summary="New source should replace the prior period summary when forced.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )

    forced = memory.consolidate(
        period_type="manual",
        period_key="manual-changing-source-set",
        project="alpha",
        force=True,
    )

    assert forced["status"] == "created"
    assert first_source.id in forced["source_memory_ids"]
    assert second_source.id in forced["source_memory_ids"]
    old_summary = memory.store.get(created["output_memory_ids"][0])
    assert old_summary is not None
    assert old_summary.superseded_by == forced["output_memory_ids"][0]


def test_python_reference_consolidate_excludes_mislabelled_secret_like_source(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    source = memory.remember(
        summary="Normal source.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )
    source.summary = "Use key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890."
    source.sensitivity = "normal"
    memory.store.put(source)

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-secret",
        project="alpha",
        dry_run=True,
    )

    assert report["candidate_count"] == 0
    assert report["outputs"] == []


def test_python_reference_consolidate_handles_sensitive_payloads_safely(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    source = memory.remember(
        summary="Medical diagnosis details must not leak.",
        details="diagnosis says migraine care plan",
        event_type="lesson",
        project="health",
        salience=0.8,
    )

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-sensitive",
        project="health",
    )

    assert report["status"] == "created"
    assert report["source_memory_ids"] == [source.id]
    output = report["outputs"][0]["memory"]
    assert output["sensitivity"] == "private"
    assert output["review"]["needs_review"] is True
    assert "diagnosis" not in output["summary"].casefold()
    assert "migraine" not in output["summary"].casefold()
    assert "diagnosis" not in output["details"].casefold()
    assert "migraine" not in output["details"].casefold()


def test_python_reference_consolidate_hides_sensitive_metadata_labels(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    source = memory.remember(
        summary="Safe summary.",
        event_type="diagnosis_lesson",
        project="private diagnosis program",
        salience=0.8,
    )

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-sensitive-metadata",
        project="private diagnosis program",
    )

    assert report["source_memory_ids"] == [source.id]
    output = report["outputs"][0]["memory"]
    assert output["sensitivity"] == "private"
    assert output["review"]["needs_review"] is True
    assert "diagnosis" not in output["summary"].casefold()
    assert "private diagnosis program" not in output["summary"].casefold()
    assert "diagnosis_lesson" not in output["summary"].casefold()
    assert "diagnosis" not in output["details"].casefold()
    assert "private diagnosis program" not in output["details"].casefold()
    assert "diagnosis_lesson" not in output["details"].casefold()


def test_python_reference_consolidate_filters_project_scope(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    alpha = memory.remember(
        summary="Alpha project should consolidate.",
        event_type="decision",
        project="alpha",
        salience=0.8,
    )
    memory.remember(
        summary="Beta project should not consolidate.",
        event_type="decision",
        project="beta",
        salience=0.8,
    )

    report = memory.consolidate(
        period_type="manual",
        period_key="manual-alpha",
        project="alpha",
    )

    assert report["candidate_count"] == 1
    assert report["source_memory_ids"] == [alpha.id]
    assert report["outputs"][0]["memory"]["project"] == "alpha"


def test_native_facade_consolidate_passes_arguments_and_decodes_json(tmp_path):
    class FakeNative:
        def __init__(self):
            self.calls = []

        def consolidate_json(self, period_type, period_key, project, dry_run, force):
            self.calls.append((period_type, period_key, project, dry_run, force))
            return json.dumps(
                {
                    "id": "con_fake",
                    "period_type": period_type,
                    "period_key": period_key,
                    "project": project,
                    "dry_run": dry_run,
                    "force": force,
                    "status": "dry_run",
                }
            )

    fake = FakeNative()
    memory = NativeTreeRingMemory(fake, tmp_path)

    report = memory.consolidate(
        period_type="weekly",
        period_key="2026-W27",
        project="alpha",
        dry_run=True,
        force=True,
    )

    assert fake.calls == [("weekly", "2026-W27", "alpha", True, True)]
    assert report["status"] == "dry_run"
    assert report["force"] is True
