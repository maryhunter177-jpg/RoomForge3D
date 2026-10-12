import { lazy, Suspense, useEffect, useState, type FormEvent } from 'react';
import { authenticate, request, type User } from './api/client';
import { useAuth } from './store/auth';
const Editor = lazy(() => import('./editor/Editor'));
const Admin = lazy(() => import('./admin/Admin'));
const Catalog = lazy(() => import('./api/Catalog'));
export function App() {
  const { session, setSession, logout } = useAuth();
  const [page, setPage] = useState<'editor' | 'catalog' | 'admin'>('editor');
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [health, setHealth] = useState<'checking' | 'online' | 'offline'>('checking');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    request<unknown>('/health', { signal: controller.signal }).then(() => setHealth('online')).catch(() => { if (!controller.signal.aborted) setHealth('offline'); });
    return () => controller.abort();
  }, []);
  const sessionToken = session?.token;
  useEffect(() => {
    if (!sessionToken) return;
    const controller = new AbortController();
    const refresh = () => { void request<User>('/api/v1/auth/me', { signal: controller.signal, headers: { Authorization: `Bearer ${sessionToken}` } }).then(user => { if (!controller.signal.aborted) setSession({ token: sessionToken, user }); }).catch(() => {}); };
    refresh();
    const timer = setInterval(refresh, 60000);
    return () => { controller.abort(); clearInterval(timer); };
  }, [sessionToken, setSession]);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setError(''); setBusy(true);
    const values = new FormData(event.currentTarget);
    try {
      const result = await authenticate(mode, String(values.get('email')).trim(), String(values.get('password')));
      const user = await request<User>('/api/v1/auth/me', { headers: { Authorization: `Bearer ${result.token}` } });
      setSession({ ...result, user });
    } catch (reason) { setError(reason instanceof Error ? reason.message : 'Não foi possível conectar à API.'); }
    finally { setBusy(false); }
  }
  if (session) return <div className="app-shell"><header className="app-header"><a className="brand" href="/" aria-label="RoomForge início"><span className="mark">R</span> RoomForge<span className="brand-sub">3D</span></a><nav aria-label="Navegação principal"><button className={page==='editor'?'selected':''} onClick={()=>setPage('editor')}>Meu ambiente</button><button className={page==='catalog'?'selected':''} onClick={()=>setPage('catalog')}>Catálogo</button>{session.user.role==='admin'&&<button className={page==='admin'?'selected':''} onClick={()=>setPage('admin')}>Administração</button>}</nav><div className="user-menu"><span>{session.user.email}</span><button onClick={()=>{logout();setPage('editor');}}>Sair</button></div></header><Suspense fallback={<p className="loading" role="status">Preparando seu espaço…</p>}>{page==='editor'?<Editor/>:page==='admin'&&session.user.role==='admin'?<Admin/>:<Catalog/>}</Suspense></div>;
  return <div className="shell">
    <header><a className="brand" href="/" aria-label="RoomForge início"><span className="mark">R</span> RoomForge<span className="brand-sub">3D</span></a><span className={`health ${health}`} role="status"><i />{health === 'online' ? 'API conectada' : health === 'offline' ? 'API indisponível' : 'Conectando à API'}</span></header>
    <main><section className="intro"><span className="eyebrow">SEU ESPAÇO. SUAS POSSIBILIDADES.</span><h1>Grandes ideias<br />começam com<br /><em>um espaço.</em></h1><p>Planeje ambientes com medidas reais. Uma base para transformar escritórios, quartos e oficinas em lugares que fazem sentido para você.</p><div className="preview" aria-label="Ilustração conceitual de uma planta, sem editor interativo"><div className="plan"><span className="dimension">4,00 m</span><div className="desk"/><div className="chair"/><div className="plant"/><div className="shelf"/><div className="rug"/></div><span className="preview-label">ESTUDO DE AMBIENTE · ILUSTRAÇÃO</span></div></section>
    <section className="account" aria-labelledby="account-title"><span className="eyebrow">ACESSO ANTECIPADO</span><><h2 id="account-title">Seu próximo espaço<br />começa aqui.</h2><p>Acesse sua conta ou cadastre-se para acompanhar a construção do RoomForge.</p><div className="tabs" aria-label="Acesso à conta"><button type="button" aria-pressed={mode === 'login'} className={mode === 'login' ? 'active' : ''} onClick={() => { setMode('login'); setError(''); }}>Entrar</button><button type="button" aria-pressed={mode === 'register'} className={mode === 'register' ? 'active' : ''} onClick={() => { setMode('register'); setError(''); }}>Criar conta</button></div><form onSubmit={submit}><label htmlFor="email">E-mail</label><input id="email" name="email" type="email" autoComplete="email" placeholder="voce@exemplo.com" required maxLength={254} /><label htmlFor="password">Senha</label><input id="password" name="password" type="password" autoComplete={mode === 'login' ? 'current-password' : 'new-password'} placeholder={mode === 'register' ? 'Pelo menos 12 caracteres' : 'Sua senha'} minLength={mode === 'register' ? 12 : 1} maxLength={128} required />{error && <p className="error" role="alert">{error}</p>}<button className="primary" disabled={busy} type="submit">{busy ? 'Conectando…' : mode === 'login' ? 'Entrar no RoomForge →' : 'Criar minha conta →'}</button></form><p className="notice">Crie seu ambiente em 3D e explore o catálogo de modelos.</p></><div className="roadmap"><span className="step current">01 <b>Fundação</b></span><span className="step">02 <b>Catálogo</b></span><span className="step">03 <b>Editor 3D</b></span></div></section></main><footer><span>Feito para imaginar. Construído para planejar.</span><span>RoomForge 3D · v0.1</span></footer>
  </div>;
}




