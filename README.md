# RoomForge 3D

Planejador de ambientes com medidas reais. Especificação completa e fases em `AGENTS.md`.

## Executar no Windows

Dê dois cliques em `Iniciar-RoomForge.cmd` e abra http://localhost:5173. Para encerrar, use `Parar-RoomForge.cmd`. O modo nativo usa PostgreSQL, MinIO, API Rust e Vite locais, sem Docker ou virtualização.

```powershell
Set-Location E:\RoomForge
.\tools\stop-native.ps1
.\tools\start-native.ps1 -Build
```

Dados persistem em `.tools/postgres-data-native` e `.tools/minio-data-native`; credenciais aleatórias em `.env.native`. Logs: `.tools/native-*.log`. As portas 55432, 8080, 5173, 9000 e 9001 são restritas ao computador local. Os scripts conferem a identidade dos processos antes de encerrá-los.

## Fases 2 e 3

Após entrar, **Meu ambiente** abre o editor Babylon.js. Crie um retângulo, um ambiente em L ou desenhe uma planta com pontos. Edite medidas, altura e espessura; consulte área e perímetro. Os botões e atalhos 1–5 alternam Topo, Frente, Lado, Isométrica e Livre. Grade, cotas e modo corte podem ser ligados e desligados. A vista Livre permite orbitar, aproximar e deslocar a câmera.

O rascunho do ambiente fica no navegador, identificado como salvamento local. A persistência de projetos na API pertence à Fase 6.

**Administração** aparece para contas com papel `admin`. No modo nativo desta instalação, `ADMIN_EMAIL` identifica a conta proprietária já cadastrada; reiniciar a API aplica essa promoção. Cadastros públicos continuam com papel `user`. Não existe senha administrativa padrão.

Envie um GLB exportado em metros, origem na base e eixo +Y para cima. A prévia gera uma miniatura PNG. O backend valida estrutura, tamanho, geometria e recursos embutidos; calcula dimensões com transformações dos nós e conta triângulos. O limite padrão é 20 MiB e 150 mil triângulos; o corpo HTTP tem limite de 50 MiB. `ASSET_MAX_BYTES` e `ASSET_MAX_TRIANGLES` configuram os limites do modelo.

O worker produz Meshopt, texturas KTX2 e dois LODs. O modelo fica em rascunho durante a revisão; só modelos processados podem ser publicados. Categorias e materiais também possuem cadastro, edição e exclusão. O catálogo público mostra apenas modelos publicados, com pesquisa, segmento e categoria.

Arquivos ficam em bucket S3 privado, com nomes por hash, cache imutável e URLs assinadas com validade de uma hora. Dois jobs são processados simultaneamente, com limite de cinco minutos por job. Reiniciar durante um processamento marca o job como interrompido; reenvie o GLB nesse caso.

## Ferramentas do pipeline

Node 22, glTF Transform 4.5.1, Meshoptimizer e KTX-Software 4.4.2. Dependências em `tools/package-lock.json`; `toktx` deve estar no PATH. MinIO nativo é compilado do release oficial `RELEASE.2025-10-15T17-29-55Z`. Ferramentas e dados locais não entram no Git.
No Windows, a instalação inicial do armazenamento pode ser repetida com `powershell -NoProfile -ExecutionPolicy Bypass -File E:\RoomForge\tools\install-minio-native.ps1 -BuildJobs 1`. O script verifica o SHA256 oficial do Go e mantém downloads, compilação, cache e temporários em `E:\RoomForge\.tools`.

Todos os arquivos de trabalho do projeto ficam no disco E. Para o uso diário, `Iniciar-RoomForge.cmd` usa os executáveis já compilados; não recompila nem reinstala as dependências a cada abertura.

```sh
npm ci --prefix tools
node tools/optimize-model.mjs input.glb output-directory
```

## Docker (alternativa)

Copie `.env.example` para `.env` e defina senhas próprias e um JWT_SECRET aleatório com pelo menos 32 caracteres.

```sh
docker compose up --build -d
docker compose down
```

Os containers exigem virtualização; o modo nativo funciona sem ela. Migrations são aplicadas pela API na inicialização. A configuração de endpoint público S3 deve corresponder ao endereço alcançável pelo navegador.

## Verificação

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
npm ci --prefix apps/web
npm test --prefix apps/web
npm run build --prefix apps/web
```

Testes SQLx exigem `DATABASE_URL` para um PostgreSQL de teste. O smoke do navegador em `apps/web/e2e/smoke.mjs` usa uma conta administrativa isolada e remove os modelos, categorias e materiais criados. Credenciais de teste devem ser fornecidas por ambiente, nunca incluídas no repositório.

## Próximas fases

4. Posicionar objetos, gizmos, snap, colisões WASM, undo/redo, portas e janelas.
5. Aplicar materiais, cores, iluminação e sombras ao ambiente.
6. Projetos na API, tutorial, exportação PNG/PDF e compartilhamento.
7. LOD por distância, instancing, cache IndexedDB, métricas e entrega.

O upload e a prévia do catálogo estão disponíveis; posicionar móveis na sala pertence à Fase 4. O gerenciamento de materiais administra o catálogo de texturas; sua aplicação na cena pertence à Fase 5.