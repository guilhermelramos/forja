# forja-module-ports

Painel direito **Portas**: portas TCP em escuta, processo dono, bancada provável
pelo cwd, abrir no navegador e encerrar com confirmação.

## O que registra

- Painel `ports.panel` em `Slot::RightPanel` (ordem 50) e contador
  `ports.status` na barra de status.
- Comandos: `ports.refresh` e `ports.kill`.
- Método RPC `ports.list` (read, idempotente).
- Configurações `ports.scan.interval_s` (0-600, padrão 30) e
  `ports.confirm.kill` (padrão ligado).
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`; o `keymap.toml` fica vazio
  porque o catálogo não tem atalho para o painel.

## Contratos

- `ports.list` devolve `{ports, scanned_at_ms, busy, error}`. A primeira chamada
  pode vir vazia e iniciar a varredura; as próximas leem o cache.
- O parser de `netstat -ano` reconhece LISTENING pelo peer `0.0.0.0:0`/`[::]:0`,
  sem ler o rótulo traduzido. `tasklist /FO CSV /NH` dá o nome do processo; CSV
  com vírgulas entre aspas é aceito.
- O cwd vem de leitura best-effort da PEB do processo (somente x64). Sem
  permissão, a porta aparece sem bancada. A bancada é o maior caminho de
  `bench.list` que contém o cwd.
- `ports.kill {pid, port?, confirm?}` recusa PID 0, 4 e o próprio processo. Com
  `ports.confirm.kill` ligado, `confirm` precisa ser `true`; o painel pede a
  confirmação em dois cliques.
- A varredura roda em `cx.spawn`, no intervalo da configuração; a barra de status
  mantém o cache vivo mesmo com o painel fechado.
- `FORJA_PORTS_FIXTURE=1` usa dados de exemplo para captura de tela.
  `FORJA_RIGHT_TAB=ports` põe o painel como primeira aba direita na captura.

## Como testar

```sh
cargo test -p forja-module-ports
cargo run -p forja-app
```

Os testes cobrem netstat em inglês e português, tasklist com vírgulas entre
aspas, ordenação/deduplicação, escolha da bancada mais específica, URL local,
recusa de encerrar o próprio processo e confirmação obrigatória.
