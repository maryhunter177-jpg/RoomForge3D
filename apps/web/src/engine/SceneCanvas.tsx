import { useEffect, useRef, useState } from 'react';
import earcut from 'earcut';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera';
import { Camera } from '@babylonjs/core/Cameras/camera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Mesh } from '@babylonjs/core/Meshes/mesh';
import { VertexData } from '@babylonjs/core/Meshes/mesh.vertexData';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder';
import { CreateLineSystem, CreateLines } from '@babylonjs/core/Meshes/Builders/linesBuilder';
import { AdvancedDynamicTexture } from '@babylonjs/gui/2D/advancedDynamicTexture';
import { TextBlock } from '@babylonjs/gui/2D/controls/textBlock';
import { type Point, validateRoom, wallOutwardNormal } from './geometry';
export type RoomView = 'top' | 'front' | 'side' | 'iso' | 'free';
export type RoomGeometry = { points: Point[]; wallHeight: number; wallThickness: number };
export type SceneCanvasProps = { room: RoomGeometry; view: RoomView; cutaway: boolean; showGrid: boolean; showDimensions: boolean; onViewChange?: (view: RoomView) => void };
export function SceneCanvas({room,view,cutaway,showGrid,showDimensions}: SceneCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [error,setError] = useState<string|null>(null);
  useEffect(() => {
    const canvas=canvasRef.current;
    if (!canvas) return;
    canvas.dataset.renderReady='false';
    const invalid=validateRoom(room.points);
    if (invalid || !Number.isFinite(room.wallHeight) || room.wallHeight < 0.2 || room.wallHeight > 10 || !Number.isFinite(room.wallThickness) || room.wallThickness < 0.02 || room.wallThickness > 1) { setError(invalid ?? 'Altura ou espessura da parede invalida.'); return; }
    setError(null);
    let engine:Engine;
    try { engine=new Engine(canvas,true,{preserveDrawingBuffer:true,stencil:true}); } catch { setError('Nao foi possivel iniciar o 3D. Ative a aceleracao grafica ou use um navegador com WebGL.'); return; }
    const scene=new Scene(engine);
    scene.clearColor=new Color4(0.045,0.071,0.09,1);
    const xs=room.points.map(p=>p.x), zs=room.points.map(p=>p.z);
    const minX=Math.min(...xs),maxX=Math.max(...xs),minZ=Math.min(...zs),maxZ=Math.max(...zs);
    const cx=(minX+maxX)/2,cz=(minZ+maxZ)/2;
    const span=Math.max(maxX-minX,maxZ-minZ,room.wallHeight,2);
    const camera=new ArcRotateCamera('room-camera',-Math.PI/4,Math.PI/3,span*1.8,new Vector3(cx,room.wallHeight*0.25,cz),scene);
    camera.minZ=0.02; camera.maxZ=1000; camera.lowerRadiusLimit=0.3; camera.upperRadiusLimit=300;
    camera.wheelDeltaPercentage=0.01; camera.panningSensibility=70; camera.inertia=0.65;
    camera.attachControl(canvas,true);
    if(view==='top') { camera.target=new Vector3(cx,0,cz); camera.alpha=-Math.PI/2; camera.beta=0.001; camera.mode=Camera.ORTHOGRAPHIC_CAMERA; }
    if(view==='front') {camera.target=new Vector3(cx,room.wallHeight/2,cz);camera.alpha=-Math.PI/2;camera.beta=Math.PI/2;}
    if(view==='side') {camera.target=new Vector3(cx,room.wallHeight/2,cz);camera.alpha=0;camera.beta=Math.PI/2;}
    if(view!=='free') { camera.lowerAlphaLimit=camera.alpha; camera.upperAlphaLimit=camera.alpha; camera.lowerBetaLimit=camera.beta; camera.upperBetaLimit=camera.beta; }
    const light=new HemisphericLight('ambient',new Vector3(-0.5,1,-0.3),scene); light.intensity=0.95;light.groundColor=new Color3(0.45,0.5,0.55);
    const floor=new Mesh('room-floor',scene), floorData=new VertexData();
    floorData.positions=room.points.flatMap(p=>[p.x,0,p.z]);floorData.indices=earcut(room.points.flatMap(p=>[p.x,p.z]));
    floorData.normals=room.points.flatMap(()=>[0,1,0]);floorData.uvs=room.points.flatMap(p=>[p.x/2,p.z/2]);floorData.applyToMesh(floor);
    const floorMaterial=new StandardMaterial('floor-material',scene);floorMaterial.diffuseColor=Color3.FromHexString('#b5c8c8');floorMaterial.specularColor=Color3.Black();floorMaterial.backFaceCulling=false;floor.material=floorMaterial;floor.freezeWorldMatrix();floorMaterial.freeze();
    const wallEntries=room.points.map((a,i)=>{
      const b=room.points[(i+1)%room.points.length], length=Math.hypot(b.x-a.x,b.z-a.z);
      const wall=CreateBox(`wall-${i}`,{width:length,height:room.wallHeight,depth:room.wallThickness},scene);
      wall.position=new Vector3((a.x+b.x)/2,room.wallHeight/2,(a.z+b.z)/2);wall.rotation.y=-Math.atan2(b.z-a.z,b.x-a.x);
      const material=new StandardMaterial(`wall-material-${i}`,scene);material.diffuseColor=Color3.FromHexString('#e8e4db');material.specularColor=Color3.Black();material.backFaceCulling=false;wall.material=material;wall.freezeWorldMatrix();
      const normal=wallOutwardNormal(room.points,i);return {wall,material,normal};
    });
    const outline=CreateLines('floor-outline',{points:[...room.points,room.points[0]].map(p=>new Vector3(p.x,0.025,p.z))},scene);outline.color=Color3.FromHexString('#38696f');outline.isPickable=false;
    if(showGrid) {
      const lines:Vector3[][]=[];const margin=1,step=0.5;
      for(let x=Math.floor((minX-margin)/step)*step;x<=maxX+margin;x+=step) lines.push([new Vector3(x,0.008,minZ-margin),new Vector3(x,0.008,maxZ+margin)]);
      for(let z=Math.floor((minZ-margin)/step)*step;z<=maxZ+margin;z+=step) lines.push([new Vector3(minX-margin,0.008,z),new Vector3(maxX+margin,0.008,z)]);
      const grid=CreateLineSystem('metric-grid',{lines},scene);grid.color=Color3.FromHexString('#507078');grid.alpha=0.4;grid.isPickable=false;
    }
    const gui=showDimensions?AdvancedDynamicTexture.CreateFullscreenUI('room-dimensions',true,scene):null;
    if(gui) room.points.forEach((a,i)=>{
      const b=room.points[(i+1)%room.points.length],n=wallEntries[i].normal,offset=0.36;
      const start=new Vector3(a.x+n.x*offset,0.05,a.z+n.z*offset),end=new Vector3(b.x+n.x*offset,0.05,b.z+n.z*offset);
      const dimension=CreateLines(`dimension-${i}`,{points:[start,end]},scene);dimension.color=Color3.FromHexString('#5bd9cf');dimension.isPickable=false;
      for(const p of [start,end]) {const tick=CreateLines(`dimension-tick-${i}`,{points:[p.add(new Vector3(n.x*-.1,0,n.z*-.1)),p.add(new Vector3(n.x*.1,0,n.z*.1))]},scene);tick.color=dimension.color;tick.isPickable=false;}
      const anchor=CreateBox(`dimension-anchor-${i}`,{size:.01},scene);anchor.position=start.add(end).scale(.5);anchor.isVisible=false;
      const label=new TextBlock(`dimension-label-${i}`,`${Math.hypot(b.x-a.x,b.z-a.z).toLocaleString('pt-BR',{maximumFractionDigits:2})} m`);label.color='#e5fffb';label.fontSize=14;label.fontWeight='600';label.outlineWidth=3;label.outlineColor='#14252b';label.width='110px';label.height='30px';gui.addControl(label);label.linkWithMesh(anchor);label.linkOffsetY=-12;
    });
    let disposed=false,raf=0,frames=0,slowFrames=0;
    const render=()=>{
      raf=0;if(disposed)return;
      for(const {wall,normal,material} of wallEntries) {
        const facing=normal.x*(camera.position.x-wall.position.x)+normal.z*(camera.position.z-wall.position.z)>0;
        material.alpha=cutaway&&(view==='top'||facing)?0.1:1;
      }
      const start=performance.now();scene.render();if(scene.isReady()){canvas.dataset.renderReady='true';canvas.dataset.cameraView=view;canvas.dataset.cameraAlpha=String(camera.alpha);canvas.dataset.cameraBeta=String(camera.beta);canvas.dataset.cameraMode=String(camera.mode);}
      if(performance.now()-start>25)slowFrames++;else slowFrames=Math.max(0,slowFrames-1);
      if(slowFrames>25&&engine.getHardwareScalingLevel()<2.5) {engine.setHardwareScalingLevel(engine.getHardwareScalingLevel()+.25);slowFrames=0;}
      if(--frames>0)raf=requestAnimationFrame(render);
    };
    const invalidate=()=>{frames=Math.max(frames,45);if(!raf&&!disposed)raf=requestAnimationFrame(render);};
    const resize=()=>{engine.resize();if(view==='top'){const aspect=canvas.clientWidth/Math.max(canvas.clientHeight,1);const half=Math.max((maxZ-minZ+2)/2,(maxX-minX+2)/2/aspect);camera.orthoLeft=-half*aspect;camera.orthoRight=half*aspect;camera.orthoTop=half;camera.orthoBottom=-half;}invalidate();};
    const observer=new ResizeObserver(resize);observer.observe(canvas);
    const events=['pointerdown','pointermove','pointerup','wheel','keydown'] as const;
    events.forEach(name=>canvas.addEventListener(name,invalidate));
    const contextLost=(event:Event)=>{event.preventDefault();setError('A conexao com a GPU foi interrompida. Recarregue a pagina para retomar o 3D.');};canvas.addEventListener('webglcontextlost',contextLost);
    resize();
    void scene.whenReadyAsync().then(()=>{if(!disposed)invalidate();});
    return()=>{disposed=true;cancelAnimationFrame(raf);observer.disconnect();events.forEach(name=>canvas.removeEventListener(name,invalidate));canvas.removeEventListener('webglcontextlost',contextLost);gui?.dispose();scene.dispose();engine.dispose();};
  },[room,view,cutaway,showGrid,showDimensions]);
  return <div style={{position:'relative',width:'100%',height:'100%',minHeight:360}}><canvas ref={canvasRef} data-testid="room-scene" aria-label="Ambiente 3D em metros" tabIndex={0} style={{width:'100%',height:'100%',display:'block',touchAction:'none',outline:'none'}}/>{error&&<div role="alert" style={{position:'absolute',inset:0,display:'grid',placeContent:'center',padding:24,background:'#14232b',color:'#fff'}}>{error}</div>}</div>;
}


