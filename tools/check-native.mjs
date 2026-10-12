import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
const base = 'http://127.0.0.1:5173';
async function request(path, options = {}) {
  const response = await fetch(base + path, { ...options, signal: AbortSignal.timeout(20000) });
  const data = await response.json().catch(() => null);
  return { status: response.status, data };
}
const health = await request('/health');
assert.equal(health.status, 200);
assert.equal(health.data.status, 'ok');
assert.equal((await fetch(base)).status, 200);
const credentials = { email: `smoke-${randomUUID()}@example.test`, password: randomUUID() + randomUUID() };
const post = (path, body) => request(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
const registered = await post('/api/v1/auth/register', credentials);
assert.equal(registered.status, 201);
assert.equal(registered.data.user.role, 'user');
assert.ok(registered.data.token);
const login = await post('/api/v1/auth/login', credentials);
assert.equal(login.status, 200);
const me = await request('/api/v1/auth/me', { headers: { Authorization: `Bearer ${login.data.token}` } });
assert.equal(me.status, 200);
assert.equal(me.data.id, registered.data.user.id);
assert.equal(me.data.email, credentials.email);
assert.equal((await post('/api/v1/auth/register', credentials)).status, 409);
assert.equal((await post('/api/v1/auth/login', { ...credentials, password: 'incorrect-password-123' })).status, 401);
assert.equal((await post('/api/v1/auth/register', { ...credentials, email: `other-${credentials.email}`, role: 'admin' })).status, 422);
assert.equal((await request('/api/v1/auth/me')).status, 401);
console.log('PASS: frontend, health, cadastro, login, perfil, duplicidade, senha incorreta e bloqueio de privilegios.');
