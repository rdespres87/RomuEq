#!/usr/bin/env python3
"""Static consistency checks for the M0.5 execution system."""
from pathlib import Path
import json, re, sys

ROOT = Path(__file__).resolve().parents[1]
TASKS = ROOT / 'docs/development/tasks/tasks.json'
required = {'id','title','status','priority','milestone','deps','decisions','allowed','forbidden','inputs','steps','tests','commands','acceptance','evidence','gate','human','rollback'}
allowed_status = {'BLOCKED','READY','IN_PROGRESS','IMPLEMENTED','TESTING','REVIEW','GATE_CHECK','DONE','FAILED'}

items=json.loads(TASKS.read_text(encoding='utf-8'))
ids=[x['id'] for x in items]
assert len(ids)==len(set(ids)), 'duplicate task ID'
idset=set(ids)
for x in items:
    missing=required-set(x)
    assert not missing, f"{x['id']}: missing fields {sorted(missing)}"
    assert x['status'] in allowed_status, f"{x['id']}: invalid status"
    for dep in x['deps']:
        assert dep in idset, f"{x['id']}: missing dependency {dep}"

# Dependency cycle detection.
graph={x['id']:x['deps'] for x in items}
visiting=set(); visited=set()
def dfs(n):
    if n in visiting: raise AssertionError(f'dependency cycle at {n}')
    if n in visited: return
    visiting.add(n)
    for d in graph[n]: dfs(d)
    visiting.remove(n); visited.add(n)
for n in graph: dfs(n)

# M0.5 must follow M0 gate; M1 must follow M0.5.
assert graph['M0.5-T001'] == ['M0-T005'], 'M0.5 must depend on M0 gate'
assert graph['M1-T001'] == ['M0.5-T008'], 'M1 must depend on M0.5 gate'
assert graph['M2-T001'] == ['M1-T005'], 'M2 must depend on M1 gate'
assert graph['M3-T001'] == ['M2-T005'], 'M3 must depend on M2 gate'

# Required hardening documents.
required_docs=[
'docs/development/MASTER_PLAN.md','docs/development/AI_DEVELOPMENT_PROTOCOL.md',
'docs/development/TASK_SPECIFICATION.md','docs/development/MILESTONE_GATE.md',
'docs/development/DECISION_REQUEST.md','docs/development/TEST_STRATEGY.md',
'docs/development/COMPATIBILITY_MATRIX.md','docs/development/PERFORMANCE_BUDGET.md',
'docs/development/TRACEABILITY_MATRIX.md','docs/development/RISK_REGISTER.md',
'docs/development/RELEASE_PROCESS.md','docs/development/EXECUTION_STATE.md','docs/development/gates/M0.5-GATE.md']
for p in required_docs:
    assert (ROOT/p).is_file(), f'missing {p}'

# Finalized M0.5 gate register must exist.
assert (ROOT/'docs/development/gates/M0.5-GATE-REGISTER.md').is_file(), 'missing M0.5 gate register'

plan=(ROOT/'docs/development/DEVELOPMENT_PLAN.md').read_text(encoding='utf-8')
assert 'M0.5' in plan, 'DEVELOPMENT_PLAN.md does not reference M0.5'

print(f'OK: {len(items)} tasks, {len(required_docs)} hardening documents, no dependency cycles.')
