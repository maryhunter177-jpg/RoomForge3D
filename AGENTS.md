# RoomForge 3D — Planejador online de ambientes (escritórios, quartos, oficinas, consultórios, clínicas, estúdios)

> Arquivo para o Codex (VS Code). Coloque na raiz do repositório como `AGENTS.md` e peça: "Execute a Fase 1". Depois, uma fase por vez.

## 1. Objetivo
Aplicação web onde o usuário monta um ambiente com medidas reais (metros), posiciona móveis/equipamentos 3D (modelos do Tripo Studio exportados em GLB), troca cor, textura de piso/parede e iluminação, e visualiza de cima, de lado e em qualquer ângulo. Um painel **Admin** cadastra e publica os modelos 3D que aparecem no catálogo dos usuários.

Referência visual: cortes isométricos (paredes recortadas, vista "casa de boneca") + planta baixa 2D com cotas.

## 2. Stack (decisões fixas)
| Camada | Tecnologia | Motivo |
|---|---|---|
| Frontend | React 18 + Vite + TypeScript + Zustand | UI rápida, estado simples |
| 3D | **Babylon.js** (`@babylonjs/core`, `loaders`, `gui`) | Gizmos, ArcRotateCamera, PBR, sombras, GLB, snapshots nativos. (Três.js só se pedirem troca.) |
| Backend | **Rust**: Axum + Tokio + SQLx (PostgreSQL) + tower-http | Rápido, seguro, binário único |
| Storage | S3 compatível (Cloudflare R2 / MinIO local) com URLs assinadas | Modelos servidos por CDN |
| Pipeline de modelos | Worker Rust (`gltf` crate) valida e extrai metadados; `gltf-transform` CLI aplica Draco/Meshopt + KTX2 + LODs | Modelos leves |
| Cálculos pesados | Crate `geometry-wasm` (Rust → WASM): snap, colisão AABB/OBB, área/perímetro, validação de cômodo | Performance no navegador |
| Auth | JWT (argon2 para senha), papéis `admin` e `user` | |
| Infra | Docker Compose (api, postgres, minio, web) | Dev em um comando |

## 3. Estrutura do monorepo
```
roomforge/
├─ AGENTS.md
├─ docker-compose.yml
├─ apps/
│  ├─ web/                  # React + Babylon
│  │  └─ src/
│  │     ├─ engine/         # SceneManager, CameraRig, Grid, Walls, Gizmos, Lighting, Materials, Picking
│  │     ├─ editor/         # Toolbar, CatalogPanel, Inspector, ViewCube, Tutorial
│  │     ├─ admin/          # Upload, lista, publicar/despublicar, categorias
│  │     ├─ store/          # useProject, useCatalog, useUI (Zustand + undo/redo)
│  │     ├─ api/            # client tipado
│  │     └─ i18n/           # pt-BR (padrão), en
│  └─ api/                  # Rust (Axum)
│     └─ src/ { main.rs, routes/, models/, services/storage.rs, services/ingest.rs, auth.rs }
├─ crates/geometry-wasm/    # Rust → WASM (wasm-pack)
├─ tools/optimize-model.sh  # gltf-transform: dedup, prune, meshopt, ktx2, simplify LODs
└─ migrations/
```

## 4. Banco de dados (migrations SQLx)
```sql
create table users(id uuid primary key default gen_random_uuid(), email text unique not null,
  password_hash text not null, role text not null check (role in ('admin','user')), created_at timestamptz default now());

create table categories(id serial primary key, slug text unique not null, name text not null,
  segment text not null); -- escritorio, quarto, oficina, consultorio, clinica, estudio

create table assets(id uuid primary key default gen_random_uuid(), name text not null,
  category_id int references categories(id), tags text[] default '{}',
  glb_url text not null, glb_lod1_url text, glb_lod2_url text, thumb_url text,
  width_m real, depth_m real, height_m real,      -- extraídos do bounding box
  triangles int, size_bytes bigint,
  placement text default 'floor' check (placement in ('floor','wall','ceiling','surface')),
  default_scale real default 1, published boolean default false,
  created_at timestamptz default now());

create table projects(id uuid primary key default gen_random_uuid(), user_id uuid references users(id),
  name text not null, data jsonb not null, thumb_url text, updated_at timestamptz default now());

create table materials(id serial primary key, kind text check (kind in ('floor','wall')),
  name text, albedo_url text, normal_url text, roughness_url text, tile_m real default 1);
```

## 5. API (Axum, prefixo `/api/v1`)
- `POST /auth/register`, `POST /auth/login`, `GET /auth/me`
- `GET /assets?category=&q=&segment=` (só `published`), `GET /assets/:id`
- **Admin** (`role=admin`): `POST /admin/assets/upload` (multipart GLB → valida → salva → job de otimização), `PATCH /admin/assets/:id`, `POST /admin/assets/:id/publish`, `DELETE /admin/assets/:id`, CRUD `/admin/categories` e `/admin/materials`
- `GET/POST/PUT/DELETE /projects`, `GET /projects/:id` (JSON do projeto)
- `GET /materials?kind=`
- Regras: rate limit, limite de upload (50 MB), validar magic bytes `glTF`, CORS restrito, logs `tracing`.

### Ingest de modelo (services/ingest.rs)
1. Receber GLB → validar com crate `gltf`.
2. Calcular bounding box (largura/profundidade/altura em metros), contagem de triângulos.
3. Rejeitar se > 150k triângulos ou > 20 MB (configurável).
4. Rodar `tools/optimize-model.sh` (meshopt + KTX2 + LOD1/LOD2).
5. Gerar thumbnail (render headless ou imagem enviada pelo admin).
6. Salvar como rascunho; admin revisa e clica **Publicar**.

## 6. Formato do projeto (JSON salvo)
```ts
type Project = {
  version: 1; unit: 'm';
  room: { points: {x:number;z:number}[]; wallHeight: number; wallThickness: number;
          floorMaterialId: number; wallMaterialId: number; wallColor: string };
  openings: { id:string; type:'door'|'window'; wallIndex:number; offset:number; width:number; height:number; sill:number }[];
  items: { id:string; assetId:string; position:[number,number,number]; rotationY:number;
           scale:number; tint?:string; materialOverrides?: Record<string,string> }[];
  lighting: { preset:'day'|'night'|'warm'|'studio'; intensity:number; sunAngle:number;
              lamps:{id:string;position:[number,number,number];color:string;intensity:number}[] };
};
```

## 7. Funcionalidades do editor (obrigatórias)
**Ambiente e medidas**
- Criar cômodo por largura × comprimento ou desenhando pontos na planta (formas em L, retângulos).
- Cotas visíveis em metros nas paredes, área (m²) e perímetro no painel; entrada numérica editável.
- Grid com snap (5/10/25 cm), snap a paredes e a outros objetos, colisão com aviso visual (vermelho).
- Portas e janelas arrastáveis sobre as paredes.

**Câmeras (botões + ViewCube + atalhos)**
- Topo (ortográfica, planta com cotas), Frente, Lado, Isométrica (corte estilo casa de boneca) e Livre (orbit/zoom/pan com ArcRotateCamera).
- **Modo corte**: paredes voltadas para a câmera ficam transparentes/rebaixadas automaticamente.
- Modo caminhada (primeira pessoa, altura 1,65 m).

**Objetos**
- Catálogo por segmento (escritório, quarto, oficina, consultório, clínica, estúdio) com busca e filtro.
- Arrastar do catálogo para a cena; Gizmos de mover/girar/escalar; duplicar, bloquear, apagar; multi-seleção.
- Undo/Redo (Ctrl+Z / Ctrl+Y) com histórico de comandos; atalhos documentados.

**Aparência**
- Troca de cor de paredes e de objetos (tint), seletor de textura de piso/parede (madeira, porcelanato, concreto, vinílico, etc.) com escala da textura em metros.
- Iluminação: presets (dia, noite, quente, estúdio), intensidade, ângulo do sol, luzes de teto/lâmpadas adicionáveis, sombras suaves.

**Experiência do usuário**
- Tutorial interativo no primeiro acesso (passo a passo com destaque nos botões) + botão "?" sempre visível + tooltips.
- Salvar automaticamente, exportar PNG (alta resolução), exportar planta PDF com cotas, compartilhar link somente-leitura.
- Interface responsiva e acessível; tema claro/escuro; idioma pt-BR por padrão.

## 8. Painel Admin
- Login admin; dashboard com total de modelos, publicados, rascunhos.
- Upload de GLB (arrastar e soltar) com pré-visualização 3D, edição de nome, categoria, tags, tipo de posicionamento (chão/parede/teto/superfície) e dimensões reais.
- Mostrar triângulos, tamanho e status da otimização; botões Publicar/Despublicar.
- Gerenciar categorias, texturas de piso/parede e presets de iluminação.

## 9. Performance (critérios não negociáveis)
- GLB com Draco/Meshopt + texturas KTX2; carregar sob demanda; LOD por distância.
- `InstancedMesh`/`thinInstances` para objetos repetidos; congelar materiais e meshes estáticos (`freezeWorldMatrix`).
- Renderizar sob demanda quando a cena está parada; `engine.setHardwareScalingLevel` adaptativo (alvo 60 fps desktop, 30+ mobile).
- Cache HTTP imutável nos GLB (hash no nome) + CDN; IndexedDB para cache local do catálogo.
- Bundle splitting: admin e editor em chunks separados; Babylon importado por módulos (tree-shaking).
- Testes: Vitest (store/geometria), `cargo test` (ingest, auth), Playwright (fluxo: criar sala → adicionar objeto → trocar piso → salvar).

## 10. Fluxo Tripo Studio → catálogo
1. Gerar modelo no Tripo → exportar **GLB** (escala em metros, origem na base central, +Y para cima).
2. Admin envia o GLB → pipeline otimiza → revisa dimensões → publica.
3. Aparece instantaneamente no catálogo dos usuários.

## 11. Fases (um prompt por fase)
1. **Fundação**: monorepo, Docker Compose, migrations, auth JWT, `/health`, CI.
2. **Admin + assets**: upload, ingest, otimização, publicar, listagem pública.
3. **Engine 3D**: cena, grid, cômodo parametrizável, paredes com cotas, 5 câmeras, modo corte.
4. **Editor**: catálogo, arrastar/soltar, gizmos, snap/colisão (WASM), undo/redo, portas e janelas.
5. **Aparência**: materiais de piso/parede, cores, iluminação, sombras.
6. **Projetos e UX**: salvar/carregar, tutorial interativo, exportar PNG/PDF, link de compartilhamento.
7. **Otimização e entrega**: LOD, instancing, testes e2e, métricas Lighthouse > 90, documentação e deploy.

## 12. Regras para o Codex
- TypeScript `strict`, Rust `clippy` sem avisos, sem `any`, sem `unwrap()` em código de produção.
- Commits pequenos por fase; README com como rodar (`docker compose up`) e `.env.example`.
- Nunca commitar segredos; validar toda entrada no backend.
- Ao concluir cada fase, listar o que foi feito, como testar e o que falta.

## Local de armazenamento autorizado
Por instrução do usuário, todos os arquivos deste projeto, ferramentas portáveis, backups, bancos de dados, caches, builds e temporários devem ficar em E:\RoomForge. Configure TEMP/TMP e caches de processos do projeto para E:. Não use C: como destino de arquivos do RoomForge. Componentes do Windows e aplicativos instalados requerem migração suportada, nunca movimentação indiscriminada de Program Files ou AppData.
