// Offline verification of pinned development/runtime source dependencies.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync, readdirSync, lstatSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
const root = fileURLToPath(new URL('..', import.meta.url));
const locks = ["fixture-source.lock.json"];
let count = 0;
for (const name of locks) {
  const receipt = JSON.parse(readFileSync(join(root, name), 'utf8'));
  assert(/^[a-f0-9]{40}$/.test(receipt.commit), 'Exact source commit required');
  for (const [path, digest] of Object.entries(receipt.files)) {
    const logical = [receipt.destination, path].filter(Boolean).join('/');
    assert(!logical.startsWith('/') && !logical.includes('\\') && logical.split('/').every(p => p && p !== '.' && p !== '..'));
    let full = root;
    for (const part of logical.split('/')) { full = join(full, part); assert(!lstatSync(full).isSymbolicLink()); }
    assert.equal('sha256:' + createHash('sha256').update(readFileSync(full)).digest('hex'), digest, logical);
    count++;
  }
  if (receipt.destination) {
    const visit = (dir, prefix = '') => readdirSync(dir, {withFileTypes:true}).flatMap(e => {
      assert(!e.isSymbolicLink());
      return e.isDirectory() ? visit(join(dir,e.name),prefix+e.name+'/') : [prefix+e.name];
    });
    assert.deepEqual(visit(join(root, receipt.destination)).sort(), Object.keys(receipt.files).sort(), 'Unpinned dependency files');
  }
}
console.log(JSON.stringify({ok:true,files:count}));
