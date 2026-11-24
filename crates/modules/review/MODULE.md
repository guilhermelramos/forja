# forja-module-review

Aba **Revisão** da bancada: pull request ou merge request da branch, checks,
threads de revisão, merge e texto gerado pelo ajudante.

## O que registra

- Aba `review.tab` em `Slot::BenchTab`.
- Comandos: `review.refresh`, `review.pr.create`, `review.merge`,
  `review.thread.reply`, `review.thread.resolve`, `review.checks.rerun`,
  `review.checks.log`, `review.generate`, `review.fix`.
- Métodos RPC: `review.refresh`, `review.pr.create`, `review.merge`,
  `review.generate`.
- Assinatura de `BenchActivated` para trocar a bancada mostrada.
- Consome `git.compare`/`git.compare.result` para exibir a base, commits,
  arquivos e totais da comparação antes do formulário.
- A base declarada pela bancada (`bench.list_cards`, `base_branch`) é aplicada
  ao resumo e à comparação; o painel mostra uma única mensagem de base ausente
  com ação direta (#172).
- Assina `git.status_changed` para atualizar a comparação após alterações da
  bancada, e `git.comparison_changed` para receber resultados em segundo plano.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`; o `keymap.toml` fica vazio
  porque o catálogo não tem atalho para a aba.

## Contratos

- O provedor vem do remoto `origin` (`api::detect`) e do host configurado nos
  balcões `counter.github.host` / `counter.gitlab.host`. O host precisa bater
  com o do balcão antes de qualquer token sair da máquina.
- O token é o mesmo que o balcão já guarda (`counter_github` / `counter_gitlab`)
  no cofre do sistema; conectar o balcão basta para revisar.
- As requisições REST/GraphQL são funções puras em `api.rs` e não fazem rede.
  `client.rs` usa o transporte e o cofre do `forja-counter-kit`.
- Ao abrir um PR, a bancada vai para `review` quando `bench.stage.pr_open` não
  for `never`; ao mesclar, vai para `ready` quando `bench.stage.pr_merge` não for
  `never`.
- O formulário "Novo pull request" mostra rótulo e campo para a branch de origem
  (somente leitura), título e descrição. Abrir sem título mantém o formulário e
  mostra o motivo no topo da aba.
- O resumo usa a branch-base e seu commit, mostra commits à frente, autor e data
  no formato do idioma, arquivos e totais `+/-`. A lista de branches permite
  escolher uma base quando não há remote ou base automática.
- A comparação de repositórios grandes roda por `Tasks` no módulo `git`; a aba
  mostra carregamento sem bloquear a interface. O botão de abrir PR explica e
  fica desabilitado quando não há commits à frente, base, remote ou provedor.
- "Ver diff desde a base" chama `diff.open {comparison: "base...HEAD"}` e
  `shell.tab.open {view_id: "diff.tab"}` para mostrar a comparação na aba
  Diferenças.
- `review.generate { kind, bench_path }` executa o modo headless do perfil do
  ajudante (`prompt.headless` / `headless_stdin` do TOML). `kind` aceita
  `commit`, `pr` e `branch`; o diff fica limitado a 12 mil caracteres.
- "Consertar com ajudante" envia o fim do log do check que falhou para
  `helper.launch`.
- Não há armazenamento próprio; o estado vive na aba e é refeito por
  `review.refresh`.
- `FORJA_REVIEW_FIXTURE=1` (ou `FORJA_FIXTURES=1`) usa `fixtures::snapshot`
  para captura de tela e demonstração sem rede.

## Como testar

```sh
cargo test -p forja-module-review
cargo run -p forja-app
```

Os testes cobrem montagem de PR/checks/merge para GitHub e GitLab, leitura das
respostas, prompts e argv do modo headless, slug de branch, contrato do kernel e
o comportamento sem bancada. O módulo `git` cobre comparação com commits à
frente, sem base e sem mudanças em repositórios temporários. A captura de tela usa
`shots/capture-review.ps1` com `FORJA_REVIEW_FIXTURE=1`.
