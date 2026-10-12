import { afterEach, describe, expect, it, vi } from 'vitest';
import { authenticate, request } from './client';
afterEach(() => vi.unstubAllGlobals());
describe('API client', () => {
  it('sends credentials using the authentication route', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ token: 'token', user: { id: '1', email: 'a@example.com', role: 'user' } }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    await authenticate('register', 'a@example.com', 'long-password');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/register', expect.objectContaining({ method: 'POST', body: JSON.stringify({ email: 'a@example.com', password: 'long-password' }) }));
  });
  it('preserves an authorization header', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    await request('/api/v1/auth/me', { headers: { Authorization: 'Bearer test' } });
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/me', expect.objectContaining({ headers: expect.objectContaining({ Authorization: 'Bearer test' }) }));
  });
  it('lets the browser provide the multipart boundary for GLB uploads', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 202 }));
    vi.stubGlobal('fetch', fetchMock);
    const data = new FormData(); data.set('name', 'Modelo');
    await request('/api/v1/admin/assets/upload', { method: 'POST', body: data, headers: { Authorization: 'Bearer test' } });
    const headers = fetchMock.mock.calls[0][1].headers as Record<string,string>;
    expect(headers.Authorization).toBe('Bearer test');
    expect(headers['Content-Type']).toBeUndefined();
  });
  it('shows a safe fallback for non-JSON gateway errors', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('<html>Bad Gateway</html>', { status: 502 })));
    await expect(request('/health')).rejects.toThrow('Não foi possível concluir. Tente novamente.');
  });
  it('propagates readable backend validation errors', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ error: 'E-mail já cadastrado' }), { status: 409 })));
    await expect(authenticate('register', 'a@example.com', 'long-password')).rejects.toThrow('E-mail já cadastrado');
  });
});

