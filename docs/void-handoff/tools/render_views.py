#!/usr/bin/env python3
"""Regenerate Markdown execution views from canonical packet JSON (no network)."""
from pathlib import Path
import json
ROOT=Path(__file__).resolve().parents[1]
def load(name): return json.loads((ROOT/'tracking'/name).read_text(encoding='utf-8'))
def save(name,text): (ROOT/name).write_text(text.rstrip()+'\n',encoding='utf-8')
def cell(v): return str(v).replace('|','\\|').replace('\n',' ')
def table(headers,rows):
 return '\n'.join(['| '+' | '.join(headers)+' |','| '+' | '.join(['---']*len(headers))+' |']+['| '+' | '.join(cell(x) for x in row)+' |' for row in rows])
def render():
 tasks=load('TASKS.json')['tasks']; tests=load('TESTS.json')['tests']; ft=load('FEATURE_TRACEABILITY.json')
 feature_by_task={t['id']:[f['id'] for f in ft['features'] if t['id'] in f['execution_tasks']] for t in tasks}
 out=['# VOID — Work packages','Generated from `tracking/TASKS.json`. Paths outside packet folders are proposed repository targets. All packages start `not_started`; no completion is inferred from a report.','## Dependency map',table(['ID','Gate','Package','Prerequisites','Status'],[(t['id'],t['phase'],t['title'],', '.join(t['depends_on']) or 'None',t['status']) for t in tasks])]
 for t in tasks:
  out += [f"## {t['id']} · {t['phase']} · {t['title']}",f"**Owner:** {t['owner']} · **Depends:** {', '.join(t['depends_on']) or 'None'} · **Status:** {t['status']}", '**Target paths:** '+', '.join('`'+p+'`' for p in t['paths']), '### Implement']
  out += [f'{i}. {s}' for i,s in enumerate(t['steps'],1)]
  out += ['### Done only when']+[f'- {s}' for s in t['done_when']]
  out += ['**Required scenarios:** '+', '.join(t['tests']), '**Feature outcomes:** '+(', '.join(feature_by_task[t['id']]) or 'Cross-cutting foundation; see mapped infrastructure and all dependent features.'),'**Dependency decisions:** '+(', '.join(t['dependency_ids']) or 'Reuse already qualified foundations; justify new dependencies separately.'),'**Evidence:** '+('; '.join(str(x) for x in t['evidence']) or 'None yet. Record actual commands, artifacts and limitations.')]
 save('WORK_PACKAGES.md','\n\n'.join(out))
 out=['# VOID — Acceptance test matrix','Generated from `tracking/TESTS.json`. These are 100 required integration scenarios, not executed tests. Also satisfy each individual feature assertion in `FEATURE_MAP.md`. Mock/protocol tests cannot replace native output or actual hardware evidence.']
 for phase in ['F0','F1','F2','F3','F4','F5']:
  out += [f'## {phase}',table(['ID / owner task','Scenario','Procedure','Expected outcome','Status'],[(t['id']+' / '+t['task_id'],t['title'],t['procedure'],t['expected'],t['status']) for t in tests if t['phase']==phase])]
 out+=['## Evidence rule','Every result needs exact steps/command, build SHA, fixture hash, OS/toolchain/device/driver/plugin/model details, observed output and result-artifact paths/hashes. Screenshots supplement numerical/audio evidence. `blocked` and `not_run` are not passes. See `schemas/evidence.schema.json` and its explicitly unexecuted example.']
 save('TEST_MATRIX.md','\n\n'.join(out))
 out=['# VOID — Feature traceability','Generated from `tracking/FEATURE_TRACEABILITY.json`. All 116 original IDs and 22 stock-tool outcome groups are preserved. Original vendor/feature descriptions are historical comparison requirements; equivalent original musical outcomes are the implementation target, not copying Apple names/assets.', '## Phase refinements']+['- '+s for s in ft['phase_refinements']]
 cats=list(dict.fromkeys(f['category'] for f in ft['features']))
 for category in cats:
  out += ['## '+category]
  for f in ft['features']:
   if f['category']!=category:continue
   out += [f"### {f['id']} · {f['feature']}",f"**Original stage/status:** {f['source_phase']} / {f['source_status']} · **Execution:** {', '.join(f['execution_tasks'])} ({', '.join(f['execution_phases'])}) · **Current:** {f['implementation_status']}", '**Required build outcome:** '+f['build'],'**Individual acceptance:** '+f['feature_specific_acceptance'],'**Source dependency note:** '+f['depends'],'**Integration tests:** '+', '.join(f['acceptance_test_ids']),'**Source/evidence IDs:** '+', '.join(f.get('sources',[])+f.get('source_repo_evidence_ids',[]))]
 out += ['## Named stock-tool coverage','Names below describe the original comparison inventory. Use original/licensed implementations and content, not copied Apple assets.',table(['ID','Family','Comparison inventory','Required musical outcome','Tasks'],[(x['id'],x['kind'],x['names'],x['implementation'],', '.join(x['execution_tasks'])) for x in ft['stock_inventory']])]
 save('FEATURE_MAP.md','\n\n'.join(out))
if __name__=='__main__':
 render();print('Generated WORK_PACKAGES.md, TEST_MATRIX.md and FEATURE_MAP.md from canonical JSON.')
