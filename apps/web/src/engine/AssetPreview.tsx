import { useEffect, useRef, useState } from 'react';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader';
import { MeshoptCompression } from '@babylonjs/core/Meshes/Compression/meshoptCompression';
import '@babylonjs/loaders/glTF';
// Meshoptimizer 1.3 decoder is shipped with the app; model previews work without CDN scripts.
MeshoptCompression.Configuration = { decoder: { url: '/decoders/meshopt_decoder.js' } };
export type AssetPreviewProps={file?:File;url?:string;onThumbnail?:(blob:Blob)=>void};
export function AssetPreview({file,url,onThumbnail}:AssetPreviewProps) {
  const canvasRef=useRef<HTMLCanvasElement>(null);
  const thumbnailCallback=useRef(onThumbnail);thumbnailCallback.current=onThumbnail;
  const [status,setStatus]=useState('Selecione um GLB para visualizar.');
  const [failed,setFailed]=useState(false);
  useEffect(()=>{
    const canvas=canvasRef.current;if(!canvas)return;
    canvas.dataset.renderReady='false';
    if(!file&&!url){setStatus('Selecione um GLB para visualizar.');setFailed(false);return;}
    let objectUrl:string|undefined;
    const source=file?(objectUrl=URL.createObjectURL(file)):url;
    let engine:Engine;
    try{engine=new Engine(canvas,true,{preserveDrawingBuffer:true,stencil:true});}catch{setStatus('WebGL indisponivel. Ative a aceleracao grafica para visualizar o modelo.');setFailed(true);if(objectUrl)URL.revokeObjectURL(objectUrl);return;}
    const scene=new Scene(engine);scene.clearColor=new Color4(.055,.085,.1,1);
    const camera=new ArcRotateCamera('asset-camera',-Math.PI/4,Math.PI/3,5,Vector3.Zero(),scene);camera.minZ=.001;camera.wheelDeltaPercentage=.01;camera.attachControl(canvas,true);
    new HemisphericLight('asset-light',new Vector3(.5,1,-.4),scene).intensity=1.1;
    let disposed=false,loaded=false,raf=0,frames=0;
    const render=()=>{raf=0;if(disposed)return;scene.render();if(loaded)canvas.dataset.renderReady='true';if(--frames>0)raf=requestAnimationFrame(render);};
    const invalidate=()=>{frames=50;if(!raf&&!disposed)raf=requestAnimationFrame(render);};
    const resize=()=>{engine.resize();invalidate();};const observer=new ResizeObserver(resize);observer.observe(canvas);
    const events=['pointerdown','pointermove','pointerup','wheel','keydown'] as const;events.forEach(event=>canvas.addEventListener(event,invalidate));
    setStatus('Carregando modelo 3D...');setFailed(false);
    const load=async()=>{
      try {
        const result=await SceneLoader.ImportMeshAsync('', '', source, scene, undefined, '.glb');
        if(disposed)return;
        result.animationGroups.forEach(group=>group.stop());await scene.whenReadyAsync();if(disposed)return;scene.render();
        let minimum=new Vector3(Infinity,Infinity,Infinity),maximum=new Vector3(-Infinity,-Infinity,-Infinity),hasMesh=false;
        for(const mesh of result.meshes) {
          if(!mesh.getTotalVertices())continue;
          mesh.computeWorldMatrix(true);const bounds=mesh.getBoundingInfo().boundingBox;
          minimum=Vector3.Minimize(minimum,bounds.minimumWorld);maximum=Vector3.Maximize(maximum,bounds.maximumWorld);hasMesh=true;
        }
        if(!hasMesh)throw new Error('O arquivo nao possui geometria visivel.');
        const size=maximum.subtract(minimum),diameter=Math.max(size.x,size.y,size.z,.05);
        camera.target=minimum.add(maximum).scale(.5);camera.alpha=-Math.PI/4;camera.beta=Math.PI/3;const aspect=canvas.clientWidth/Math.max(canvas.clientHeight,1);const halfFov=Math.min(camera.fov/2,Math.atan(Math.tan(camera.fov/2)*aspect));camera.radius=size.length()/2/Math.sin(halfFov)*1.15;camera.lowerRadiusLimit=diameter*.15;camera.upperRadiusLimit=diameter*10;camera.maxZ=diameter*100;camera.minZ=Math.max(diameter/10000,.0001);
        await scene.whenReadyAsync();if(disposed)return;
        loaded=true;resize();camera.getViewMatrix(true);scene.render();canvas.dataset.renderReady='true';setStatus('');
        canvas.toBlob(blob=>{if(blob&&!disposed)thumbnailCallback.current?.(blob);},'image/png');
      }catch(error){if(!disposed){setFailed(true);setStatus(error instanceof Error?`Nao foi possivel carregar o GLB: ${error.message}`:'Nao foi possivel carregar o modelo.');}}
    };
    void load();
    return()=>{disposed=true;cancelAnimationFrame(raf);observer.disconnect();events.forEach(event=>canvas.removeEventListener(event,invalidate));scene.dispose();engine.dispose();if(objectUrl)URL.revokeObjectURL(objectUrl);};
  },[file,url]);
  return <div style={{position:'relative',height:280,minHeight:200,width:'100%',background:'#11232a',borderRadius:12,overflow:'hidden'}}><canvas ref={canvasRef} data-testid="asset-preview" aria-label="Pre-visualizacao 3D do modelo GLB" tabIndex={0} style={{width:'100%',height:'100%',display:'block',touchAction:'none'}}/>{status&&<p role={failed?'alert':'status'} style={{position:'absolute',inset:0,display:'grid',placeContent:'center',padding:20,margin:0,color:'#e3f5f4',background:'#11232acc'}}>{status}</p>}</div>;
}






