import { readFile, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root = new URL('../', import.meta.url);
const settings = (await readFile(new URL('.env.native', root), 'utf8')).replace(/^\uFEFF/,'');
const env = Object.fromEntries(settings.split(/\r?\n/).filter(line => /^[A-Z_][A-Z0-9_]*=/.test(line)).map(line => {
 const separator = line.indexOf('=');
 let value = line.slice(separator+1).trim();
 if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) value = value.slice(1,-1);
 return [line.slice(0,separator),value];
}));
if (!env.DATABASE_URL) throw new Error('DATABASE_URL is missing in .env.native');
const database = new URL(env.DATABASE_URL);
const email=`phase23-${Date.now()}@example.test`, password=randomBytes(24).toString('hex');
const response=await fetch('http://127.0.0.1:8080/api/v1/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password}),signal:AbortSignal.timeout(30000)});
if(response.status!==201)throw new Error(`register ${response.status}`);
const session=await response.json();
if(!session.user || typeof session.user.id!=='string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(session.user.id))throw new Error('Invalid test account ID from API');
const result=spawnSync(fileURLToPath(new URL('.tools/pgsql/bin/psql.exe',root)),[
 '-h',database.hostname,'-p',database.port||'5432','-U',decodeURIComponent(database.username),'-d',decodeURIComponent(database.pathname.slice(1)),
 '-v','ON_ERROR_STOP=1','-c',`UPDATE users SET role='admin' WHERE id='${session.user.id}'`,
],{env:{...process.env,PGPASSWORD:decodeURIComponent(database.password)},encoding:'utf8',windowsHide:true,timeout:30000});
if(result.status!==0)throw new Error('Test admin promotion failed');
await writeFile(new URL('.tools/e2e-account.json',root),JSON.stringify({email,password,id:session.user.id}));
console.log('Conta isolada para validação criada.');
