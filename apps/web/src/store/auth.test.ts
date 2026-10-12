import { beforeEach, expect, it } from 'vitest';
import { useAuth } from './auth';
beforeEach(() => useAuth.getState().logout());
it('clears token and user together when leaving the account', () => {
  useAuth.getState().setSession({ token: 'private-token', user: { id: '1', email: 'a@example.com', role: 'user' } });
  expect(useAuth.getState().session?.token).toBe('private-token');
  useAuth.getState().logout();
  expect(useAuth.getState().session).toBeNull();
});
