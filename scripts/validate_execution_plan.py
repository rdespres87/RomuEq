#!/usr/bin/env python3
import json, pathlib, re, sys
root=pathlib.Path(__file__).resolve().parents[1]
tasks=json.loads((root/'docs/development/tasks/tasks.json').read_text())
errors=[]
ids=[t.get('id') for t in tasks]
if len(tasks)!=68: errors.append(f'expected 68 tasks, got {len(tasks)}')
if len(set(ids))!=len(ids): errors.append('duplicate task IDs')
idset=set(ids)
for t in tasks:
    tid=t.get('id','?')
    for dep in t.get('deps',[]):
        if dep not in idset: errors.append(f'{tid}: missing dependency {dep}')
    if not tid.startswith('M0'):
        required=['purpose','preconditions','implementation_steps','validation_commands','acceptance_criteria','allowed_paths','forbidden_paths','evidence_artifacts','stop_the_line','rollback_failure_handling','done_definition','delegation_notes']
        for k in required:
            if not t.get(k): errors.append(f'{tid}: missing {k}')
        pairs=[('steps','implementation_steps'),('commands','validation_commands'),('acceptance','acceptance_criteria'),('evidence','evidence_artifacts'),('allowed','allowed_paths'),('forbidden','forbidden_paths'),('rollback','rollback_failure_handling')]
        for a,b in pairs:
            if t.get(a)!=t.get(b): errors.append(f'{tid}: {a} != {b}')
# cycle detection
g={t['id']:t.get('deps',[]) for t in tasks}; seen=set(); active=set()
def dfs(n):
    if n in active: errors.append('dependency cycle detected at '+n); return
    if n in seen:return
    active.add(n)
    for d in g.get(n,[]): dfs(d)
    active.remove(n); seen.add(n)
for n in g: dfs(n)
manual=(root/'docs/development/TASK_EXECUTION_MANUAL.md').read_text()
if '{", ".join' in manual or '{t[' in manual: errors.append('manual contains generation placeholders')
ready=[t for t in tasks if t.get('status')=='READY']
if len(ready)!=1: errors.append(f'expected exactly one READY task, got {[t["id"] for t in ready]}')
for t in ready:
    missing=[d for d in t.get('deps',[]) if next(x for x in tasks if x['id']==d).get('status')!='DONE']
    if missing: errors.append(f'{t["id"]}: READY with unfinished deps {missing}')
state=(root/'docs/development/EXECUTION_STATE.md').read_text()
if 'M1-T001' not in state: errors.append('EXECUTION_STATE does not mention M1-T001 current action')
if errors:
    print('FAIL')
    print('\n'.join('- '+e for e in errors)); sys.exit(1)
print('OK: 68 tasks, enriched M1-M11 schema, no cycles, manual placeholders absent, exactly one executable READY task.')
