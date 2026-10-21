'use strict';
// Sustituye dependencias vulnerables dentro del árbol de npm por versiones corregidas.
// Uso: node apply_patches.js <npmRoot> <patches.json> <registry>
const fs = require('fs');
const path = require('path');
const os = require('os');
const cp = require('child_process');

const [, , npmRoot, patchFile, registry] = process.argv;
const patches = JSON.parse(fs.readFileSync(patchFile, 'utf8'));

function parse(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?/.exec(v);
  if (!m) throw new Error('versión no válida: ' + v);
  return { n: [+m[1], +m[2], +m[3]], pre: m[4] || null };
}
function cmp(a, b) {
  const x = parse(a), y = parse(b);
  for (let i = 0; i < 3; i++) if (x.n[i] !== y.n[i]) return x.n[i] < y.n[i] ? -1 : 1;
  if (x.pre && !y.pre) return -1;
  if (!x.pre && y.pre) return 1;
  return 0;
}
function* childPackages(dir) {
  const nm = path.join(dir, 'node_modules');
  if (!fs.existsSync(nm)) return;
  for (const e of fs.readdirSync(nm, { withFileTypes: true })) {
    if (e.name.startsWith('.') || !(e.isDirectory() || e.isSymbolicLink())) continue;
    if (e.name.startsWith('@')) {
      for (const s of fs.readdirSync(path.join(nm, e.name))) yield path.join(nm, e.name, s);
    } else yield path.join(nm, e.name);
  }
}
function* allPackages(dir) {
  for (const p of childPackages(dir)) { yield p; yield* allPackages(p); }
}
function resolvable(fromDir, dep) {
  let d = fromDir;
  for (;;) {
    if (fs.existsSync(path.join(d, 'node_modules', dep, 'package.json'))) return true;
    const up = path.dirname(d);
    if (up === d || d.length <= npmRoot.length - 1) return false;
    d = up;
  }
}

let failed = false;
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'chs-patch-'));
for (const patch of patches) {
  for (const dir of [...allPackages(npmRoot)]) {
    const pj = path.join(dir, 'package.json');
    if (!fs.existsSync(pj)) continue;
    const meta = JSON.parse(fs.readFileSync(pj, 'utf8'));
    if (meta.name !== patch.name) continue;
    const major = String(parse(meta.version).n[0]);
    const target = patch.fixes_by_major[major];
    if (!target || cmp(meta.version, target) >= 0) continue;
    const dest = fs.mkdtempSync(path.join(tmp, 'p-'));
    const tgz = cp.execFileSync('npm', ['pack', `${patch.name}@${target}`, '--silent', '--registry', registry, '--pack-destination', dest],
      { encoding: 'utf8' }).trim().split('\n').pop();
    cp.execFileSync('tar', ['-xzf', path.join(dest, tgz), '-C', dest]);
    const fresh = path.join(dest, 'package');
    const newMeta = JSON.parse(fs.readFileSync(path.join(fresh, 'package.json'), 'utf8'));
    if (newMeta.version !== target) throw new Error(`versión descargada inesperada ${newMeta.version} != ${target}`);
    const nested = path.join(dir, 'node_modules');
    const keep = fs.existsSync(nested) ? path.join(dest, 'keep_nm') : null;
    if (keep) fs.renameSync(nested, keep);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.cpSync(fresh, dir, { recursive: true });
    if (keep) fs.renameSync(keep, nested);
    for (const dep of Object.keys(newMeta.dependencies || {})) {
      if (!resolvable(dir, dep)) {
        console.error(`CHS_PATCH_INCOMPATIBLE ${patch.name}@${target} requiere ${dep}, que no existe en el árbol de npm`);
        failed = true;
      }
    }
    console.log(`CHS_PATCH ${patch.name} ${meta.version} -> ${target} (${path.relative(npmRoot, dir)})`);
  }
}
// verificación final: ninguna copia debe quedar por debajo de su versión corregida
for (const patch of patches) {
  for (const dir of allPackages(npmRoot)) {
    const pj = path.join(dir, 'package.json');
    if (!fs.existsSync(pj)) continue;
    const meta = JSON.parse(fs.readFileSync(pj, 'utf8'));
    if (meta.name !== patch.name) continue;
    const target = patch.fixes_by_major[String(parse(meta.version).n[0])];
    if (target && cmp(meta.version, target) < 0) {
      console.error(`CHS_PATCH_UNRESOLVED ${patch.name}@${meta.version} en ${dir}`);
      failed = true;
    }
  }
}
fs.rmSync(tmp, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
