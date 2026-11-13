# Atlas

O atlas é a documentação visual da Forja: cada tela, em cada idioma e tema, com a captura, a árvore de elementos (papel, nome, estado e retângulo de cada widget) e uma página que descreve tudo o que aparece.

As páginas geradas ficam no branch **`atlas`**: `README.md` com o progresso, `pages/`, `captures/` e `PROBLEMAS.md`.

## Como funciona
1. `cargo xtask atlas capture --exe <forja-app.exe> --out <dir>` lista as telas do app (`forja-app --dump-catalog`). Depois abre cada uma com `--screenshot` e `--ui-tree`, e grava `captures/` e `manifest.json`. Telas com pixels idênticos são marcadas com `same_as` e documentadas uma vez.
2. `cargo xtask atlas status --out <dir>` compara o manifesto com `pages/` e grava `todo.json`. Ali ficam as telas sem página e as páginas desatualizadas: a captura mudou ou algum elemento com nome não é citado.
3. O workflow `.github/workflows/atlas.yml` roda na nuvem, num runner Windows do GitHub Actions. Para cada item do `todo.json`, ele chama o agente (OpenCode, modelo gratuito) com `atlas/agent/INSTRUCTIONS.md`, a captura e o `atlas brief`. Em seguida valida a resposta com `atlas accept`, regenera o índice (`atlas index`) e faz push no branch `atlas`.
4. O loop termina quando `todo.json` diz `complete: true`. Depois disso, cada mudança nas telas deixa páginas desatualizadas, e o agente as refaz.

## Rodar localmente
```powershell
cargo build -p forja-app
cargo xtask atlas capture --exe target\debug\forja-app.exe --out atlas-out --locales pt-BR --themes dark
cargo xtask atlas status --out atlas-out
cargo xtask atlas brief --out atlas-out --state pt-BR.dark.rail.board.page
```

## Configuração no GitHub
- Secret `OPENCODE_API_KEY` (opcional): chave do OpenCode. Os modelos gratuitos podem exigir login.
- Variável `ATLAS_MODEL`: padrão `opencode/glm-5.3-flash-free`. A reserva, `ATLAS_FALLBACK_MODEL`, tem padrão `opencode/deepseek-v4.1-flash-free`.
