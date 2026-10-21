'use strict';
// Suite funcional ejecutada DENTRO de la imagen endurecida (sin shell; solo Node.js).
// Cada prueba emite una línea "CHS_RESULT {json}". La plataforma exige que se ejecuten todas las esperadas.
const fs = require('fs');
const os = require('os');
const path = require('path');
const cp = require('child_process');
const tls = require('tls');
const https = require('https');

const E = __EXPECT__;
const results = [];

function emit(id, status, detail) {
  const r = { id, status, detail: String(detail === undefined ? '' : detail).slice(0, 500) };
  results.push(r);
  console.log('CHS_RESULT ' + JSON.stringify(r));
}
async function test(id, fn) {
  try {
    const d = await fn();
    emit(id, 'pass', d);
  } catch (e) {
    emit(id, 'fail', e && e.message ? e.message : e);
  }
}
function assert(cond, msg) { if (!cond) throw new Error(msg); }
function run(cmd, args, opts) {
  return cp.execFileSync(cmd, args, Object.assign({ encoding: 'utf8', timeout: 120000 }, opts || {})).trim();
}
function exists(p) { return fs.existsSync(p); }
function spawnMissing(cmd) {
  const r = cp.spawnSync(cmd, ['--version'], { encoding: 'utf8' });
  return r.error && r.error.code === 'ENOENT';
}
const npmCli = '/usr/local/lib/node_modules/npm/bin/npm-cli.js';
function npm(args, opts) {
  // Sin shell no existe /usr/bin/env: se invoca npm-cli.js con el propio node.
  return E.shell ? run('npm', args, opts) : run(process.execPath, [npmCli, ...args], opts);
}

function* walk(dir) {
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
  for (const e of entries) {
    const p = path.join(dir, e.name);
    if (e.isSymbolicLink()) continue;
    if (e.isDirectory()) yield* walk(p); else yield p;
  }
}

(async () => {
  await test('runtime_version', () => {
    assert(process.version === 'v' + E.node_version, `esperado v${E.node_version}, obtenido ${process.version}`);
    return process.version;
  });
  await test('js_execution', () => {
    const out = run(process.execPath, ['-e', 'console.log([1,2,3].map(x=>x*2).reduce((a,b)=>a+b))']);
    assert(out === '12', 'salida inesperada: ' + out);
    return 'ok';
  });
  if (E.non_root) {
    await test('non_root', () => {
      assert(process.getuid() !== 0, 'el proceso se ejecuta como root');
      assert(process.getuid() === E.uid, `uid esperado ${E.uid}, obtenido ${process.getuid()}`);
      return 'uid=' + process.getuid();
    });
  }
  await test('workdir_writable', () => {
    const f = path.join(E.workdir, '.chs-write-test');
    fs.writeFileSync(f, 'x');
    fs.unlinkSync(f);
    return E.workdir;
  });
  await test('no_setuid', () => {
    const bad = [];
    for (const root of ['/bin', '/sbin', '/usr', '/lib', '/etc', '/opt']) {
      for (const f of walk(root)) {
        try { if (fs.lstatSync(f).mode & 0o6000) bad.push(f); } catch { /* ignorar */ }
      }
    }
    assert(bad.length === 0, 'binarios setuid/setgid: ' + bad.slice(0, 5).join(', '));
    return 'ninguno';
  });
  await test('shell_state', () => {
    const has = exists('/bin/sh');
    assert(has === E.shell, E.shell ? '/bin/sh debería existir' : '/bin/sh existe pero se pidió eliminarla');
    return has ? 'presente' : 'ausente';
  });
  await test('pkg_manager_state', () => {
    const has = exists('/sbin/apk') || exists('/usr/bin/apk');
    assert(has === E.pkg_manager, E.pkg_manager ? 'apk debería existir' : 'apk existe pero se pidió eliminarlo');
    return has ? 'presente' : 'ausente';
  });
  await test('npm_state', () => {
    const has = exists('/usr/local/lib/node_modules/npm/package.json');
    assert(has === E.npm, E.npm ? 'npm debería existir' : 'npm existe pero se pidió eliminarlo');
    if (!E.npm) assert(spawnMissing('npm'), 'el comando npm responde aunque se pidió eliminarlo');
    if (E.npm && E.shell) assert(exists('/usr/local/bin/npm'), 'falta el enlace /usr/local/bin/npm');
    if (E.npx) assert(exists('/usr/local/bin/npx'), 'falta npx');
    if (!E.npx && E.npm) assert(!exists('/usr/local/bin/npx'), 'npx existe aunque se pidió eliminarlo');
    return has ? 'presente' : 'ausente';
  });
  if (E.ca_certs) {
    await test('ca_bundle', () => {
      assert(tls.rootCertificates.length > 50, 'Node.js no dispone de certificados raíz');
      assert(exists('/etc/ssl/certs/ca-certificates.crt'), 'falta /etc/ssl/certs/ca-certificates.crt');
      return `${tls.rootCertificates.length} certificados`;
    });
    if (E.network) {
      await test('https_request', () => new Promise((resolve, reject) => {
        const req = https.get(E.npm_registry, { timeout: 20000 }, (res) => {
          res.resume();
          if (res.statusCode && res.statusCode < 500) resolve('HTTP ' + res.statusCode); else reject(new Error('HTTP ' + res.statusCode));
        });
        req.on('timeout', () => req.destroy(new Error('tiempo de espera agotado')));
        req.on('error', reject);
      }));
    }
  }
  if (E.npm) {
    await test('npm_version', () => {
      const v = npm(['--version']);
      assert(/^\d+\.\d+\.\d+/.test(v), 'versión de npm inesperada: ' + v);
      return v;
    });
    if (E.network && E.install_deps) {
      await test('npm_install', () => {
        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chs-npm-'));
        fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name: 'chs-smoke', version: '1.0.0', private: true }));
        const env = Object.assign({}, process.env, { npm_config_cache: path.join(dir, '.cache'), npm_config_update_notifier: 'false' });
        npm(['install', '--no-audit', '--no-fund', '--ignore-scripts', '--registry', E.npm_registry, 'is-number@7.0.0'], { cwd: dir, env });
        assert(exists(path.join(dir, 'node_modules/is-number/package.json')), 'is-number no se instaló');
        return 'is-number@7.0.0 instalado';
      });
    }
  }
  for (const tool of E.tools) {
    await test('tool_' + tool, () => run(tool, ['--version']).split('\n')[0]);
  }
  if (E.build_tools && E.npm) {
    await test('native_toolchain', () => {
      const gcc = run('gcc', ['--version']).split('\n')[0];
      const py = run('python3', ['--version']);
      const gyp = run(process.execPath, ['/usr/local/lib/node_modules/npm/node_modules/node-gyp/bin/node-gyp.js', '--version']);
      return `${gcc}; ${py}; node-gyp ${gyp}`;
    });
  }
  console.log('CHS_SUMMARY ' + JSON.stringify({ total: results.length, passed: results.filter((r) => r.status === 'pass').length }));
})();
