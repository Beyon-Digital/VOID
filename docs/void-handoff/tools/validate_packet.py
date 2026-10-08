#!/usr/bin/env python3
"""Offline packet integrity/traceability check. This is NOT an application test runner."""
from pathlib import Path
import argparse, hashlib, json, sys
ROOT=Path(__file__).resolve().parents[1]
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--delivery-hashes',action='store_true',help='Also verify original delivery hashes; implementation edits should invalidate these.');args=ap.parse_args()
 problems=[]; checks=[]
 def check(ok,msg):
  (checks if ok else problems).append(msg)
 def read(p):return json.loads((ROOT/p).read_text(encoding='utf-8'))
 try:
  tasks=read('tracking/TASKS.json')['tasks']; tests=read('tracking/TESTS.json')['tests']; features=read('tracking/FEATURE_TRACEABILITY.json'); source=read('references/PARITY_SOURCE.json');deps=read('tracking/DEPENDENCIES.json')['libraries'];infra=read('tracking/INFRASTRUCTURE.json')['requirements']
  ti={x['id']:x for x in tasks}; xi={x['id']:x for x in tests}; di={x['id']:x for x in deps}
  check(len(tasks)==30 and len(ti)==30,'30 unique work packages')
  check(len(tests)==100 and len(xi)==100,'100 unique acceptance scenarios')
  check(len(features['features'])==116 and {x['id'] for x in features['features']}=={x['id'] for x in source['features']},'All 116 source feature IDs preserved')
  check(len(features['stock_inventory'])==len(source['stockInventory'])==22,'All 22 stock-tool groups preserved')
  check([(x['kind'],x['names'],x['implementation']) for x in features['stock_inventory']]==[(x['kind'],x['names'],x['implementation']) for x in source['stockInventory']], 'Stock comparison names and required outcomes preserved verbatim')
  for x in features['stock_inventory']:check(bool(x['execution_tasks']) and all(i in ti for i in x['execution_tasks']), x['id']+' assigned to existing work')
  check(len(di)==len(deps)==41,'41 distinct dependency decisions')
  check(len(infra)==20,'20 infrastructure requirements')
  for t in tasks:
   check(all(i in ti for i in t['depends_on']),t['id']+' valid prerequisite IDs')
   check(all(i in xi and xi[i]['task_id']==t['id'] for i in t['tests']),t['id']+' owns valid acceptance scenarios')
   check(all(i in di for i in t['dependency_ids']),t['id']+' valid dependency IDs')
   check(bool(t['steps']) and bool(t['done_when']),t['id']+' implementation and done conditions present')
   if t['status']=='development_ready':check(bool(t.get('readiness_evidence')) and bool(t.get('readiness_target')),t['id']+' development readiness has target-specific evidence')
   if t['status']=='complete':check(bool(t['evidence']) and all(xi[i]['status']=='passed' and xi[i]['evidence'] for i in t['tests']),t['id']+' completed work has evidence')
  seen=set(); active=set()
  def visit(i):
   if i in active:raise ValueError('Task dependency cycle at '+i)
   if i in seen:return
   active.add(i)
   for p in ti[i]['depends_on']:visit(p)
   active.remove(i);seen.add(i)
  for i in ti:visit(i)
  checks.append('Dependency graph is acyclic')
  for f in features['features']:
   check(bool(f['execution_tasks']) and all(i in ti for i in f['execution_tasks']),f['id']+' assigned to existing work')
   check(bool(f['feature_specific_acceptance']) and all(i in xi for i in f['acceptance_test_ids']),f['id']+' has individual assertion and valid tests')
   if f['implementation_status']=='complete':check(bool(f['evidence']),f['id']+' completion has evidence')
  for x in tests:
   check(x['task_id'] in ti and x['id'] in ti[x['task_id']]['tests'],x['id']+' is not orphaned')
   if x['status']=='passed':check(bool(x['evidence']),x['id']+' pass has evidence')
  for x in infra:check(all(i in ti for i in x['execution_tasks']),x['id']+' valid task mapping')
  for x in deps:
   if x['qualification_status']=='qualified':check(bool(x['pinned_revision']) and bool(x['license_approval']) and bool(x['evidence']),x['id']+' qualification has lock/approval/evidence')
  required=['README.md','HANDOFF.md','CONTRACTS.md','KICKOFF.md','WORK_PACKAGES.md','TEST_MATRIX.md','FEATURE_MAP.md','PROGRESS.md','SOURCES.md','prompts/01_FOUNDATION.md','prompts/06_SPECIALIST_PARITY.md']
  check(all((ROOT/p).is_file() for p in required),'Required documents exist')
  for p in ROOT.rglob('*.json'):
   json.loads(p.read_text(encoding='utf-8'))
  checks.append('Every packet JSON file parses')
  if args.delivery_hashes:
   manifest=ROOT/'MANIFEST.sha256';check(manifest.exists(),'Delivery manifest exists')
   if manifest.exists():
    for line in manifest.read_text().splitlines():
     expected,name=line.split('  ',1);p=(ROOT/name).resolve()
     check(p.is_relative_to(ROOT) and p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest()==expected,'Delivery hash '+name)
 except (KeyError,ValueError,OSError,TypeError) as exc:problems.append(str(exc))
 print(json.dumps({'scope':'Packet integrity only; no VOID build/audio/AI/hardware tests executed','checks_passed':len(checks),'errors':problems,'result':'PASS' if not problems else 'FAIL'},indent=2))
 return 1 if problems else 0
if __name__=='__main__':sys.exit(main())
