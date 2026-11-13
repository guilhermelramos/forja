# forja-module-privacy

Módulo `privacy` da Forja: política local de histórico, área de transferência,
logs, cofre de segredos, uso, ajudante e navegador.

## O que registra

- Configurações `privacy.*` do catálogo (histórico, clipboard, logs, cofre,
  uso, ajudante e navegador);
- Comandos `privacy.policy`, `privacy.redact`, `privacy.clipboard.schedule_clear`,
  `privacy.secrets.status` e `privacy.browser.policy`;
- Os mesmos cinco métodos RPC;
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

- Publica `privacy.policy` com a política resolvida, `privacy.redact` para
  ocultar tokens/senhas, `privacy.clipboard.schedule_clear` para limpar a área
  de transferência, `privacy.secrets.status` para o cofre e
  `privacy.browser.policy` para cookies e saída. Tabela em `design/contracts.md`.
- A limpeza da área de transferência usa a API do Windows (`OpenClipboard`,
  `EmptyClipboard`, `SetClipboardData`) e só roda quando o prazo é maior que
  zero. Testes usam o caminho desligado e nunca tocam no clipboard real.
- Nenhuma configuração sai da máquina; o módulo não faz rede nem telemetria.

## Como testar

```sh
cargo test -p forja-module-privacy
```
