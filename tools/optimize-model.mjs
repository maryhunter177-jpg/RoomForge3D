import { NodeIO } from '@gltf-transform/core';
import { ALL_EXTENSIONS } from '@gltf-transform/extensions';
import { dedup, prune, weld, simplify, meshopt } from '@gltf-transform/functions';
import { MeshoptDecoder, MeshoptEncoder, MeshoptSimplifier } from 'meshoptimizer';
import draco3d from 'draco3dgltf';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const [input, output] = process.argv.slice(2);
if (!input || !output) throw new Error('Uso: node tools/optimize-model.mjs input.glb output-directory');
const io = new NodeIO().registerExtensions(ALL_EXTENSIONS).registerDependencies({
  'meshopt.decoder': MeshoptDecoder, 'meshopt.encoder': MeshoptEncoder,
  'draco3d.decoder': await draco3d.createDecoderModule(),
  'draco3d.encoder': await draco3d.createEncoderModule()
});
await Promise.all([MeshoptDecoder.ready, MeshoptEncoder.ready, MeshoptSimplifier.ready]);
await mkdir(output, { recursive: true });
const base = join(output, 'prepared.glb');
const ktx = join(output, 'prepared-ktx.glb');
let document = await io.read(resolve(input));
await document.transform(dedup(), weld(), prune());
await io.write(base, document);
const textures = document.getRoot().listTextures();
if (textures.some(texture => texture.getMimeType() !== 'image/ktx2')) {
  const cli = join(dirname(fileURLToPath(import.meta.url)), 'node_modules', '@gltf-transform', 'cli', 'bin', 'cli.js');
  await new Promise((accept, reject) => {
    const child = spawn(process.execPath, [cli, 'uastc', base, ktx, '--level', '2', '--zstd', '18'], { stdio: ['ignore', 'inherit', 'inherit'], windowsHide: true });
    child.on('error', reject);
    child.on('exit', code => code === 0 ? accept() : reject(new Error(`KTX2 falhou: ${code}`)));
  });
} else {
  await io.write(ktx, document);
}
for (const [name, ratio] of [['model', 1], ['lod1', 0.5], ['lod2', 0.2]]) {
  document = await io.read(ktx);
  if (ratio < 1) await document.transform(simplify({ simplifier: MeshoptSimplifier, ratio, error: 0.01 }));
  await document.transform(meshopt({ encoder: MeshoptEncoder, level: 'high' }), prune());
  await io.write(join(output, `${name}.glb`), document);
}
await writeFile(join(output, 'optimization.json'), JSON.stringify({ version: 1, compression: 'meshopt', textureFormat: 'ktx2', lodRatios: [1, 0.5, 0.2] }));
await Promise.all([rm(base, { force: true }), rm(ktx, { force: true })]);
console.log('Modelos Meshopt e LODs preparados; texturas convertidas para KTX2 quando presentes.');