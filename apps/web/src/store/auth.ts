import { create } from 'zustand';
import type { Session } from '../api/client';
type AuthState = { session: Session | null; setSession: (session: Session) => void; logout: () => void };
// Tokens stay in memory: reloading requires a new login, avoiding browser-storage exposure.
export const useAuth = create<AuthState>((set) => ({ session: null, setSession: (session) => set({ session }), logout: () => set({ session: null }) }));
