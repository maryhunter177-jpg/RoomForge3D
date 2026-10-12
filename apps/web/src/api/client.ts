export type User = { id: string; email: string; role: 'user' | 'admin' };
export type Session = { token: string; user: User };
export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(path, { ...options, headers: { ...(options.body instanceof FormData ? {} : { 'Content-Type': 'application/json' }), ...options.headers } });
  const payload: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const message = typeof payload === 'object' && payload !== null && 'error' in payload && typeof payload.error === 'string' ? payload.error : 'Não foi possível concluir. Tente novamente.';
    throw new Error(message);
  }
  return payload as T;
}
export function authenticate(mode: 'login' | 'register', email: string, password: string) {
  return request<Session>(`/api/v1/auth/${mode}`, { method: 'POST', body: JSON.stringify({ email, password }) });
}

