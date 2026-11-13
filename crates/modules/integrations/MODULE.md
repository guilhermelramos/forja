# forja-module-integrations

Módulo `integrations` da Forja: o webhook de saída (`integrations.webhook.*`).

## O que registra

- configurações `integrations.webhook.enabled` (liga o envio), `integrations.webhook.url`
  (`http://` ou `https://`) e `integrations.webhook.events` (lista de nomes de evento);
- assinantes dos eventos de contrato `stage.changed` (`StageChanged`) e
  `bench.created` (`BenchCreated`), declarados com `contract_event`;
- bundles i18n em en e pt-BR.

Não tem comandos, views nem atalhos.

## Como envia

Com o webhook ligado, a URL válida e o nome do evento na lista, o evento vai por
`POST` como JSON `{"event": <nome>, "at": <ms>, "data": <evento>}`. O envio roda em
`Ctx::spawn` (módulo `webhook.rs`), com tempo máximo de 10 s. Falha de rede ou status
fora de 2xx vira aviso (`notify.show`, nível de erro), nunca some em silêncio.

## Contratos

- Consome `stage.changed` e `bench.created` (`design/contracts.md`). Não publica nada.
- Usa `ureq` do `[workspace.dependencies]`.

## Pendente

- Proxy HTTP: quando o contrato `network.proxy` (Lote 1) estiver na `main`, o envio
  deve passar por ele. Hoje o `ureq` usa a configuração padrão do sistema.

## Como testar

```sh
cargo test -p forja-module-integrations
```

O teste de envio sobe um servidor de uma requisição em `127.0.0.1` e confere o
método, o corpo JSON e o tratamento de status. Nenhum teste toca a rede externa.
