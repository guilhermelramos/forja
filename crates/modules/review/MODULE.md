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
o comportamento sem bancada. A captura de tela usa
`shots/capture-review.ps1` com `FORJA_REVIEW_FIXTURE=1`.
