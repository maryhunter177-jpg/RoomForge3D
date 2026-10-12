import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { NodeIO } from '@gltf-transform/core';
import { ALL_EXTENSIONS } from '@gltf-transform/extensions';
import { MeshoptDecoder } from 'meshoptimizer';
await MeshoptDecoder.ready;
const io=new NodeIO().registerExtensions(ALL_EXTENSIONS).registerDependencies({'meshopt.decoder':MeshoptDecoder});
const directory=resolve(process.argv[2]??'.tools/demo-optimized');
let lastTriangles=Infinity;
for(const name of ['model','lod1','lod2']){
 const path=join(directory,`${name}.glb`),bytes=await readFile(path);
 assert.equal(bytes.readUInt32LE(0),0x46546c67);
 const json=JSON.parse(bytes.subarray(20,20+bytes.readUInt32LE(12)).toString());
 assert(json.extensionsRequired?.includes('EXT_meshopt_compression'),`${name}: compression must be required`);
 assert(json.extensionsUsed?.includes('KHR_texture_basisu'),`${name}: BasisU textures required`);
 const document=await io.read(path);
 const triangles=document.getRoot().listMeshes().reduce((sum,mesh)=>sum+mesh.listPrimitives().reduce((count,primitive)=>count+(primitive.getIndices()?.getCount()??primitive.getAttribute('POSITION').getCount())/3,0),0);
 assert(triangles>0&&triangles<lastTriangles,`${name}: LOD must reduce actual triangle count`);lastTriangles=triangles;
 const textures=document.getRoot().listTextures();assert(textures.length>0);
 for(const texture of textures){assert.equal(texture.getMimeType(),'image/ktx2');assert.deepEqual(Array.from(texture.getImage().subarray(0,12)),[0xab,0x4b,0x54,0x58,0x20,0x32,0x30,0xbb,0x0d,0x0a,0x1a,0x0a]);}
 console.log(JSON.stringify({name,triangles,bytes:bytes.length,meshopt:true,ktx2:true}));
}
