# forja-module-ports

Painel direito **Portas**: lista compacta e agrupada por bancada das portas TCP
em escuta da Forja (pelo diretório de trabalho ou árvore de processos do
terminal/ajudante), filtro por texto, interruptor para exibir portas do
sistema, abrir portas HTTP no navegador e encerrar com confirmação ou recusa
humana para processos do sistema.

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
- Por padrão o painel exibe apenas portas associadas a bancadas/projetos da
  Forja ou abertas na árvore de processos da Forja; o interruptor
  "Mostrar todas as portas do sistema" revela os demais processos e serviços.
- As portas são agrupadas por bancada (seguidas por Forja, Outros processos e
  Sistema), com filtro por texto e botão "Abrir no navegador" apenas para portas
  HTTP.
- O parser de `netstat -ano` reconhece LISTENING pelo peer `0.0.0.0:0`/`[::]:0`,
  sem ler o rótulo traduzido. `tasklist /FO CSV /NH` dá o nome do processo e a
  sessão (`Services`/`0` identifica serviços do Windows).
- O cwd e o PID pai vêm de `NtQueryInformationProcess` (`ProcessBasicInformation`)
  e leitura da PEB (x64); a árvore de ancestrais associa processos filhos de
  terminais/ajudantes à bancada correspondente.
- `ports.kill {pid, port?, confirm?}` recusa PID 0, 4, o próprio processo e
  processos do sistema com mensagem humana (`forja_kernel::human_error`). Com
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
aspas e classificação de serviços, ordenação/deduplicação, escolha da bancada
mais específica, inspeção da árvore de processos, heurística HTTP, agrupamento e
filtro, URL local, recusa humana de processos do sistema/próprio processo e
confirmação obrigatória.
