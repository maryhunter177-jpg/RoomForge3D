import { Document, NodeIO } from '@gltf-transform/core';
import sharp from 'sharp';
import { mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
const output=resolve(process.argv[2]??'.tools/demo-model.glb');
const document=new Document(),buffer=document.createBuffer(),scene=document.createScene('RoomForge desk');
const width=128,height=128,pixels=new Uint8Array(width*height*3);
for(let y=0;y<height;y++)for(let x=0;x<width;x++){const grain=Math.sin(y*.65+x*.05)*13+Math.sin(y*2.4)*5;const i=(y*width+x)*3;pixels[i]=155+grain;pixels[i+1]=112+grain;pixels[i+2]=72+grain;}
const image=await sharp(pixels,{raw:{width,height,channels:3}}).png().toBuffer();
const texture=document.createTexture('embedded wood grain').setImage(image).setMimeType('image/png');
const wood=document.createMaterial('Oak').setBaseColorTexture(texture).setRoughnessFactor(.78).setMetallicFactor(0);
const metal=document.createMaterial('Charcoal steel').setBaseColorFactor([.055,.07,.08,1]).setRoughnessFactor(.4).setMetallicFactor(.75);
const ceramic=document.createMaterial('Lamp shade').setBaseColorFactor([.85,.82,.68,1]).setRoughnessFactor(.6);
function geometry(name,positions,normals,uvs,indices,material,translation){
  const primitive=document.createPrimitive().setAttribute('POSITION',document.createAccessor().setType('VEC3').setArray(new Float32Array(positions)).setBuffer(buffer)).setAttribute('NORMAL',document.createAccessor().setType('VEC3').setArray(new Float32Array(normals)).setBuffer(buffer)).setAttribute('TEXCOORD_0',document.createAccessor().setType('VEC2').setArray(new Float32Array(uvs)).setBuffer(buffer)).setIndices(document.createAccessor().setType('SCALAR').setArray(new Uint32Array(indices)).setBuffer(buffer)).setMaterial(material);
  scene.addChild(document.createNode(name).setMesh(document.createMesh(name).addPrimitive(primitive)).setTranslation(translation));
}
function box(name,w,h,d,material,translation){
 const faces=[[[1,0,0],[[1,-1,-1],[1,1,-1],[1,1,1],[1,-1,1]]],[[-1,0,0],[[-1,-1,1],[-1,1,1],[-1,1,-1],[-1,-1,-1]]],[[0,1,0],[[-1,1,-1],[-1,1,1],[1,1,1],[1,1,-1]]],[[0,-1,0],[[-1,-1,1],[-1,-1,-1],[1,-1,-1],[1,-1,1]]],[[0,0,1],[[1,-1,1],[1,1,1],[-1,1,1],[-1,-1,1]]],[[0,0,-1],[[-1,-1,-1],[-1,1,-1],[1,1,-1],[1,-1,-1]]]];
 const positions=[],normals=[],uvs=[],indices=[];
 for(const [normal,vertices] of faces){const start=positions.length/3;for(const [x,y,z]of vertices){positions.push(x*w/2,y*h/2,z*d/2);normals.push(...normal);}uvs.push(0,0,0,1,1,1,1,0);indices.push(start,start+1,start+2,start,start+2,start+3);}
 geometry(name,positions,normals,uvs,indices,material,translation);
}
function sphere(name,radius,translation){
 const positions=[],normals=[],uvs=[],indices=[],columns=64,rows=32;
 for(let y=0;y<=rows;y++)for(let x=0;x<=columns;x++){const phi=y/rows*Math.PI,theta=x/columns*Math.PI*2,n=[Math.sin(phi)*Math.cos(theta),Math.cos(phi),Math.sin(phi)*Math.sin(theta)];positions.push(...n.map(v=>v*radius));normals.push(...n);uvs.push(x/columns,y/rows);}
 for(let y=0;y<rows;y++)for(let x=0;x<columns;x++){const a=y*(columns+1)+x,b=a+columns+1;if(y>0)indices.push(a,a+1,b);if(y<rows-1)indices.push(a+1,b+1,b);}
 geometry(name,positions,normals,uvs,indices,ceramic,translation);
}
box('Wood tabletop',1.5,.06,.75,wood,[0,.75,0]);
for(const x of [-.64,.64])for(const z of [-.27,.27])box('Steel leg',.045,.72,.045,metal,[x,.36,z]);
box('Rear brace',1.28,.045,.04,metal,[0,.3,.27]);
box('Drawer',.42,.14,.5,wood,[.4,.63,0]);
box('Lamp stem',.025,.34,.025,metal,[-.52,.95,.12]);
box('Lamp base',.17,.025,.17,metal,[-.52,.795,.12]);
sphere('Curved lamp shade',.12,[-.52,1.13,.12]);
await mkdir(dirname(output),{recursive:true});await new NodeIO().write(output,document);
console.log(`Modelo de demonstracao salvo: ${output}`);
