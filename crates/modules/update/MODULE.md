# forja-module-update

Módulo `update` da Forja: canal, frequência e política de download, com
verificação de versão local. Não há telemetria nem chamada de rede.

## O que registra

- Configurações `update.channel.name`, `update.check.frequency` e
  `update.download.auto`;
- Comandos `update.check` e `update.status`;
- Os mesmos dois métodos RPC;
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

- `update.check` recebe `{latest_version?}` e compara a versão informada pelo
  chamador (que já a obteve localmente) com `CARGO_PKG_VERSION`. `startup`
  verifica uma vez por sessão, `daily`/`weekly` respeitam o intervalo e
  `never` desliga.
- `update.status` devolve canal, frequência, download automático, versão atual
  e a hora da última verificação local.
- Nada é enviado pela rede; a versão mais recente é sempre um dado de entrada.

## Como testar

```sh
cargo test -p forja-module-update
```
