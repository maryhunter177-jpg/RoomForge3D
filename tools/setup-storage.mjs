import { Client } from 'minio';
const endpoint = new URL(process.env.STORAGE_ENDPOINT);
const client = new Client({ endPoint: endpoint.hostname, port: Number(endpoint.port || (endpoint.protocol === 'https:' ? 443 : 80)), useSSL: endpoint.protocol === 'https:', accessKey: process.env.STORAGE_ACCESS_KEY, secretKey: process.env.STORAGE_SECRET_KEY });
const bucket = process.env.STORAGE_BUCKET;
if (!(await client.bucketExists(bucket))) await client.makeBucket(bucket, process.env.STORAGE_REGION || 'us-east-1');
// MinIO configures API CORS globally with MINIO_API_CORS_ALLOW_ORIGIN at startup.
// Native startup restricts that setting to this application's localhost origin.
if (['127.0.0.1', 'localhost'].includes(endpoint.hostname)) {
  const origin='http://localhost:5173';
  const preflight=await fetch(new URL(`/${encodeURIComponent(bucket)}`,endpoint),{method:'OPTIONS',headers:{Origin:origin,'Access-Control-Request-Method':'GET'},signal:AbortSignal.timeout(10000)});
  if(!preflight.ok||preflight.headers.get('access-control-allow-origin')!==origin)throw new Error('CORS MinIO incorreto: inicie com MINIO_API_CORS_ALLOW_ORIGIN=http://localhost:5173.');
  const forbidden=await fetch(new URL(`/${encodeURIComponent(bucket)}`,endpoint),{method:'OPTIONS',headers:{Origin:'https://untrusted.example','Access-Control-Request-Method':'GET'},signal:AbortSignal.timeout(10000)});
  if(forbidden.headers.has('access-control-allow-origin'))throw new Error('MinIO esta aceitando uma origem CORS externa.');
}
console.log('Bucket privado S3 pronto; CORS local verificado.');
