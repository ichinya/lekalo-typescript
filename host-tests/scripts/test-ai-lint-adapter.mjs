import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFileSync, writeFileSync, unlinkSync} from 'node:fs';
import {createRequire} from 'node:module';
import {join} from 'node:path';
import {canonical, digest, environment, golden, known, root, run, schema} from './lib/ai-lint-contract-gate.mjs';

const require = createRequire(process.env.LEKALO_AJV_NODE_PATH + '/');
const Ajv = require('ajv/dist/2020').default;
const ajv = new Ajv({strict:true, allErrors:true});
const read = file => JSON.parse(readFileSync(join(root,file),'utf8'));
const binary = process.env.LEKALO_BIN || join(root,'target/debug',process.platform==='win32'?'lekalo.exe':'lekalo');
const clone = structuredClone;
function raw(project,args) {
  const child=spawnSync(binary,['--json','--no-cache',...args],{cwd:project,encoding:'utf8',maxBuffer:16*1024*1024,timeout:60000});
  assert.ifError(child.error);return {child,envelope:JSON.parse(child.stdout.trim()||child.stderr.trim())};
}
function refusal(env,args,id,status='invalid') { const r=run(env.project,args,status);assert.ok(r.envelope.diagnostics.some(d=>d.id===id),JSON.stringify(r.envelope));return r; }

const env = environment();
const update = process.argv.includes('--update');
try {
  const protocol=ajv.compile(read('contracts/target-protocol.schema.v0.6.4.json')),frozen=ajv.compile(read('contracts/target-protocol.schema.v0.3.2.json'));
  const [target, runtime, path, bindings] = ['node-typescript',process.execPath,'src/events.ts',[['src/events.ts#Task','planner.task','entity'],['src/events.ts#focus','planner.focus_task','command'],['src/events.ts#edit','planner.edit_task_cmd','command']]];
  const adapterHome=join(root,'tests/external/lekalo-typescript/adapters/node-typescript');
  const adapter=join(adapterHome,'test/native-test-entry.mjs'),bytes=readFileSync(join(env.project,path));
  const request={protocol:'lekalo.target/v1',protocol_version:'0.6.4',request_id:'req-'+'7'.repeat(64),operation:'lint',project_root:'.',target,limits:{max_output_bytes:8388608,timeout_ms:600000},lint_request:{scope:['planner'],pins:env.pins,bindings:bindings.map(([nativeId,symbol,kind])=>({path,nativeId,symbol,kind,fingerprint:digest(bytes)})),files:[path]}};
  assert.ok(protocol(request),JSON.stringify(protocol.errors));assert.equal(frozen(request),false);
  const call=r=>spawnSync(runtime,[adapter],{cwd:env.project,input:canonical(r),encoding:'utf8',timeout:60000,maxBuffer:16*1024*1024});
  const child=call(request);assert.equal(child.status,0,child.stderr);const response=JSON.parse(child.stdout);assert.ok(protocol(response),JSON.stringify(protocol.errors));const e=response.result.lint_evidence;schema('evidence',e);
  assert.ok(e.records.some(r=>r.kind==='effect'&&r.mechanism==='observer'&&r.activation.length===5),`${target} actual observer chain`);assert.ok(e.records.some(r=>r.kind==='reflection'),`${target} dynamic call`);assert.ok(e.coverage.every(c=>c.state!=='complete'));
  const live=run(env.project,[...env.args,'--evidence',env.file(`${target}-live.json`,e)]).report;
  assert.ok(live.findings.some(f=>f.ruleId==='hidden.observer-write'&&f.semanticSymbol.value==='planner.focus_task'));
  assert.ok(!live.findings.some(f=>f.ruleId==='hidden.observer-write'&&f.semanticSymbol.value==='planner.edit_task_cmd'),'declared effect control');
  assert.ok(live.findings.some(f=>f.ruleId==='hidden.reflective-call'));assert.equal(live.summary.verifiedEffects,0);
  // Compiler/runtime pins are part of custody, so golden producer receipts are
  // schema vectors; live receipts are always regenerated and admitted above.
  const snapshot=clone(e);snapshot.producer.compiler=known('typescript/7.0.2');golden(`adapter-${target}`,snapshot,update);
  const duplicateBinding=clone(request);duplicateBinding.lint_request.bindings.push({...duplicateBinding.lint_request.bindings[0],symbol:'planner.edit_task_cmd'});assert.notEqual(call(duplicateBinding).status,0,'ambiguous native join is refused');
  const old=clone(request);old.protocol_version='0.3.2';assert.notEqual(call(old).status,0);
  const extra=clone(request);extra.lint_request.extra=true;assert.notEqual(call(extra).status,0);
  const nullRequest=clone(request);nullRequest.lint_request=null;assert.notEqual(call(nullRequest).status,0);
  const traversal=clone(request);traversal.lint_request.files=['../host.ts'];assert.notEqual(call(traversal).status,0);
  // A same-spelled API without recognized receiver/registration is not a hook.
  const original=bytes.toString('utf8'),negative=original.replace("bus.on('saved',", "// bus.on('saved',");
  writeFileSync(join(env.project,path),negative);const negRequest=clone(request);negRequest.lint_request.bindings.forEach(b=>b.fingerprint=digest(negative));const nc=call(negRequest);assert.equal(nc.status,0,nc.stderr);assert.ok(!JSON.parse(nc.stdout).result.lint_evidence.records.some(r=>r.kind==='effect'));writeFileSync(join(env.project,path),bytes);
  const install=raw(env.project,['adapter','install','path:'+adapterHome,'--dry-run']);assert.equal(install.child.status,0,JSON.stringify(install.envelope));
  const plan=install.envelope.plan?.planId??install.envelope.planId??install.envelope.plan?.id;assert.ok(plan,JSON.stringify(install.envelope));
  const applied=raw(env.project,['adapter','install','path:'+adapterHome,'--confirm',plan]);assert.equal(applied.child.status,0,JSON.stringify(applied.envelope));
  const scan=run(env.project,[...env.args,'--scan-target',target,'--source-file',path]);assert.ok(scan.report.coverage.some(c=>c.target===target));assert.equal(scan.report.summary.verifiedEffects,0);
  const observed={schemaVersion:'lekalo/observed-scan/v0.2.16',adapter:{id:'lekalo-target-'+target,version:'0.4.0',digest:null},project:'planner',revision:digest(bytes),symbols:bindings.map(([nativeId,symbol,kind])=>({id:symbol,kind,stableKey:nativeId,location:{path,line:1},fingerprint:digest(bytes),mappingConfidence:'high',evidence:{}})),endpoints:[],schemas:[]};
  const updated=raw(env.project,['observe','update','--scan',env.file('observed-scan.json',observed)]);assert.equal(updated.child.status,0,JSON.stringify(updated.envelope));
  for(const [,symbol]of bindings){const confirmed=raw(env.project,['observe','confirm',symbol]);assert.equal(confirmed.child.status,0,JSON.stringify(confirmed.envelope));}
  const indexPath=join(env.project,'.lekalo/import/observed/index.json'),beforeIndex=readFileSync(indexPath),beforeSource=readFileSync(join(env.project,path));
  const collected=run(env.project,[...env.args,'--scan-target',target,'--source-file',path,'--spans']);
  assert.ok(collected.report.findings.some(f=>f.ruleId==='hidden.observer-write'&&f.semanticSymbol.value==='planner.focus_task'),target+' installed production collector positive');
  assert.ok(!collected.report.findings.some(f=>f.ruleId==='hidden.observer-write'&&f.semanticSymbol.value==='planner.edit_task_cmd'));
  assert.ok(collected.report.findings.some(f=>f.ruleId==='hidden.reflective-call'));assert.equal(collected.report.summary.verifiedEffects,0);
  const selected=raw(env.project,['ai-lint','--symbol','planner.focus_task',...env.args,'--scan-target',target,'--source-file',path]);assert.equal(selected.child.status,0,JSON.stringify(selected.envelope));assert.ok(selected.envelope.report.findings.some(f=>f.ruleId==='hidden.observer-write'&&f.semanticSymbol.value==='planner.focus_task'),target+' single-symbol scope retains entity mapping');
  assert.deepEqual(readFileSync(indexPath),beforeIndex,'lint retains observed index');assert.deepEqual(readFileSync(join(env.project,path)),beforeSource,'lint retains source');
  unlinkSync(indexPath);
  // R1: exactly 32/33 detector locations, through both admission paths.
  try {for(const count of [32,33]) {
    const text=Array.from({length:count},(_,i)=>`export function case${i}(obj: any, key: string) { obj[key](); }`).join('\n')+'\n';
    writeFileSync(join(env.project,path),text);
    const boundary=clone(request);boundary.lint_request.bindings=[];
    const receipt=call(boundary);assert.equal(receipt.status,0,receipt.stderr);const response=JSON.parse(receipt.stdout);assert.ok(protocol(response),JSON.stringify(protocol.errors));
    const evidence=response.result.lint_evidence;schema('evidence',evidence);assert.equal(evidence.locations.length,32);assert.equal(evidence.limitations.includes('document-location-limit'),count===33);
    const expectedRecords=32;assert.equal(evidence.records.length,expectedRecords,'whole retained records');
    const supplied=run(env.project,[...env.args,'--evidence',env.file('bounded-collector.json',evidence)]),installed=run(env.project,[...env.args,'--scan-target',target,'--source-file',path]);
    const selected=report=>report.findings.filter(f=>f.target===target).map(f=>[f.id,f.ruleId,f.confidence,f.severity]);
    assert.deepEqual(selected(installed.report),selected(supplied.report));assert.equal(selected(supplied.report).length,expectedRecords);
    assert.deepEqual(installed.report.coverage.filter(c=>c.target===target),supplied.report.coverage.filter(c=>c.target===target),'collection/replay coverage parity');
    for(const result of [supplied,installed]) {
      assert.ok(result.envelope.diagnostics.some(d=>d.id==='ai-lint.coverage-incomplete'));
      const coverage=result.report.coverage.find(c=>c.target===target&&c.rule==='hidden.reflective-call');assert.equal(coverage.state,'partial');assert.equal(coverage.limitations.includes('document-location-limit'),count===33);
    }
    const oversized=clone(evidence);oversized.locations.push(clone(oversized.locations[0]));schema('evidence',oversized,false);refusal(env,[...env.args,'--evidence',env.file('over-bound-collector.json',oversized)],'ai-lint.input-invalid');
  }} finally {writeFileSync(join(env.project,path),bytes);}
} finally {
  env.close();
}
