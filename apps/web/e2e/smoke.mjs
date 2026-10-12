import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
const tempDir=fileURLToPath(new URL('../../../.tools/temp/',import.meta.url));
process.env.TEMP=tempDir;process.env.TMP=tempDir;
process.env.PLAYWRIGHT_BROWSERS_PATH=fileURLToPath(new URL('../../../.tools/playwright-browsers/',import.meta.url));
await mkdir(tempDir,{recursive:true});
const {chromium}=await import('@playwright/test');
const artifactsDir=new URL('./artifacts/',import.meta.url);
const artifactPath=name=>fileURLToPath(new URL(name,artifactsDir));
const baseURL=process.env.ROOMFORGE_SMOKE_URL??'http://localhost:5173';
const account=await readFile(new URL('../../../.tools/e2e-account.json',import.meta.url),'utf8').then(text=>JSON.parse(text.replace(/^\uFEFF/,''))).catch(()=>({}));
const email=process.env.ROOMFORGE_SMOKE_EMAIL??account.email;
const password=process.env.ROOMFORGE_SMOKE_PASSWORD??account.password;
if(!email||!password)throw new Error('Configure ROOMFORGE_SMOKE_EMAIL e ROOMFORGE_SMOKE_PASSWORD, sem incluir credenciais no comando.');
const edgePath='C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const executablePath=process.env.ROOMFORGE_BROWSER_PATH??(existsSync(edgePath)?edgePath:undefined);
const browser=await chromium.launch({headless:true,...(executablePath?{executablePath}:{}),args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const page=await browser.newPage({viewport:{width:1440,height:980}});
page.setDefaultTimeout(60000);page.setDefaultNavigationTimeout(120000);
const browserErrors=[];page.on('pageerror',error=>browserErrors.push(error.message));
const assertReady=async(testid)=>{await page.locator(`[data-testid="${testid}"][data-render-ready="true"]`).waitFor({timeout:60000});};
const unique=`Smoke model ${Date.now()}`;let assetId=null;let token=null;let categoryId=null;let materialId=null;
await mkdir(artifactsDir,{recursive:true});
try{
 await page.goto(baseURL);
 await page.getByLabel('E-mail',{exact:true}).fill(email);
 await page.getByLabel('Senha',{exact:true}).fill(password);
 const loginResponse=page.waitForResponse(response=>response.url().endsWith('/auth/login')&&response.request().method()==='POST');
 await page.getByRole('button',{name:'Entrar no RoomForge'}).click();
 const login=await loginResponse;assert.equal(login.status(),200,'Login failed');const auth=await login.json();assert.equal(typeof auth.token,'string');token=auth.token;
 await page.getByRole('button',{name:'Meu ambiente',exact:true}).waitFor();
 await assertReady('room-scene');
 await page.getByLabel('Largura (m)',{exact:true}).fill('6');
 await page.getByLabel('Comprimento (m)',{exact:true}).fill('5');
 await page.getByRole('button',{name:'Atualizar ambiente'}).click();
 await assertReady('room-scene');
 assert.equal(await page.locator('.metrics strong').first().textContent(),'30.00');
 for(const [name,view,beta] of [['Topo','top',.001],['Frente','front',Math.PI/2],['Lado','side',Math.PI/2],['Isométrica','iso',Math.PI/3],['Livre','free',Math.PI/3]]){await page.getByRole('button',{name,exact:true}).click();await page.waitForFunction(({view,beta})=>{const canvas=document.querySelector('[data-testid=room-scene]');return canvas?.dataset.renderReady==='true'&&canvas.dataset.cameraView===view&&Math.abs(Number(canvas.dataset.cameraBeta)-beta)<.005;},{view,beta});}
 await page.keyboard.press('1');await page.waitForFunction(()=>{const c=document.querySelector('[data-testid=room-scene]');return c?.dataset.cameraView==='top'&&c.dataset.cameraMode==='1'&&Math.abs(Number(c.dataset.cameraBeta)-.001)<.005;});await page.getByRole('button',{name:'Topo',exact:true}).getAttribute('aria-pressed').then(value=>assert.equal(value,'true'));
 await page.getByRole('button',{name:'Corte',exact:true}).click();await assertReady('room-scene');
 await page.getByRole('button',{name:'Grade',exact:true}).click();await assertReady('room-scene');
 await page.getByRole('button',{name:'Cotas',exact:true}).click();await assertReady('room-scene');
 await page.getByLabel('Formato',{exact:true}).selectOption('l');await page.getByRole('button',{name:'Atualizar ambiente'}).click();await assertReady('room-scene');assert.equal(await page.locator('.metrics strong').first().textContent(),'22.50');
 await page.getByLabel('Formato',{exact:true}).selectOption('polygon');await page.getByRole('button',{name:'Limpar',exact:true}).click();
 const drawing=page.getByRole('img',{name:'Planta: clique para adicionar pontos'});const box=await drawing.boundingBox();assert.ok(box);
 for(const [x,z] of [[1,1],[5,1],[5,4],[1,4]])await drawing.click({position:{x:x/10*box.width,y:z/7.5*box.height}});
 await page.getByRole('button',{name:'Atualizar ambiente'}).click();await assertReady('room-scene');assert.equal(await page.locator('.metrics strong').first().textContent(),'12.00');
 await page.getByRole('button',{name:'Corte',exact:true}).click();await page.getByRole('button',{name:'Grade',exact:true}).click();await page.getByRole('button',{name:'Cotas',exact:true}).click();await assertReady('room-scene');await page.screenshot({path:artifactPath('room-editor.png'),fullPage:true});
 await page.setViewportSize({width:390,height:844});await assertReady('room-scene');await page.screenshot({path:artifactPath('room-editor-mobile.png'),fullPage:true});await page.setViewportSize({width:1440,height:980});
 await page.getByRole('button',{name:'Catálogo',exact:true}).click();await page.getByRole('heading',{name:'Catálogo de modelos'}).waitFor();
 if(auth.user.role!=='admin')throw new Error('Smoke de administração requer conta admin. Editor concluído; administração não executada.');
 await page.getByRole('button',{name:'Administração',exact:true}).click();await page.getByRole('heading',{name:'Seu catálogo, organizado.'}).waitFor();
 await page.getByRole('button',{name:'Categorias',exact:true}).click();const categoryForm=page.locator('form').first();await categoryForm.getByLabel('Nome da categoria').fill(unique);await categoryForm.getByLabel('Slug da categoria').fill(`smoke-${Date.now()}`);const createCategory=page.waitForResponse(r=>r.url().endsWith('/admin/categories')&&r.request().method()==='POST');await categoryForm.getByRole('button',{name:'Criar categoria'}).click();const categoryResponse=await createCategory;assert.equal(categoryResponse.status(),201,'Category creation failed');categoryId=(await categoryResponse.json()).id;
 await page.getByRole('button',{name:'Materiais',exact:true}).click();const materialForm=page.locator('form').first();await materialForm.getByLabel('Nome do material').fill(unique);const createMaterial=page.waitForResponse(r=>r.url().endsWith('/admin/materials')&&r.request().method()==='POST');await materialForm.getByRole('button',{name:'Criar material'}).click();const materialResponse=await createMaterial;assert.equal(materialResponse.status(),201,'Material creation failed');materialId=(await materialResponse.json()).id;
 await page.getByRole('button',{name:'Modelos',exact:true}).click();
 const model=await readFile(new URL('../../../.tools/demo-model.glb',import.meta.url));await page.getByLabel('Arquivo GLB').setInputFiles({name:'smoke-cube.glb',mimeType:'model/gltf-binary',buffer:model});await assertReady('asset-preview');await page.getByText('Miniatura PNG gerada automaticamente.').waitFor({timeout:30000});
 await page.getByLabel('Nome',{exact:true}).fill(unique);await page.getByLabel('Categoria',{exact:true}).selectOption(String(categoryId));await page.getByLabel('Tags (separadas por vírgula)').fill('smoke, cube');
 const upload=page.waitForResponse(r=>r.url().endsWith('/admin/assets/upload')&&r.request().method()==='POST');await page.getByRole('button',{name:'Enviar para processamento'}).click();const uploadResponse=await upload;assert.equal(uploadResponse.status(),202,'GLB upload failed');const uploaded=await uploadResponse.json();assetId=uploaded.id;assert.equal(typeof assetId,'string');
 const article=page.locator('article.admin-asset').filter({has:page.getByRole('heading',{name:unique,exact:true})});await article.getByRole('button',{name:'Publicar',exact:true}).waitFor({timeout:60000});
 await page.waitForFunction(name=>{const card=[...document.querySelectorAll('article.admin-asset')].find(element=>element.querySelector('h3')?.textContent===name);return card&&[...card.querySelectorAll('button')].some(button=>button.textContent==='Publicar'&&!button.disabled);},unique,{timeout:180000});
 await article.getByRole('button',{name:'Publicar',exact:true}).click();await article.getByText('Publicado',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Catálogo',exact:true}).click();await page.getByRole('heading',{name:unique,exact:true}).waitFor({timeout:30000});await page.getByRole('button').filter({has:page.getByRole('heading',{name:unique,exact:true})}).click();await assertReady('asset-preview');await page.screenshot({path:artifactPath('catalog-model.png'),fullPage:true});await page.getByRole('button',{name:'Fechar visualização'}).click();
 assert.deepEqual(browserErrors,[]);console.log('PASS: login, room rectangle/L/polygon, 5 views, shortcuts, cutaway/grid/dimensions, category/material creation, GLB preview+thumbnail+upload+processing+publication+public catalog preview.');
}catch(error){
 await page.screenshot({path:artifactPath('failure.png'),fullPage:true}).catch(()=>{});
 throw error;
}finally{
 try {
  if(token){
   const headers={Authorization:`Bearer ${token}`};
   for(const [path,id]of [['assets',assetId],['materials',materialId],['categories',categoryId]]){
    if(!id)continue;
    try {const response=await page.request.delete(`${baseURL}/api/v1/admin/${path}/${id}`,{headers,timeout:10000});if(!response.ok())console.warn(`Cleanup incomplete: ${path} ${response.status()}`);}
    catch {console.warn(`Cleanup unavailable for ${path}; API may have stopped.`);}
   }
  }
 }finally{await browser.close();}
}






