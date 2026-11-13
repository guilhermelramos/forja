# forja-module-network

Módulo `network` da Forja: política de proxy, exceções de hosts, autenticação de proxy, verificação TLS, tempo limite HTTP e modo offline.

## O que registra

- Configurações: `network.proxy.mode`, `network.proxy.url`, `network.proxy.bypass`, `network.proxy.auth`, `network.tls.verify`, `network.http.timeout_s`, `network.mode.offline`;
- Comandos e métodos RPC: `network.proxy` e `network.check`.

## Contratos

- Publica `network.proxy` (`NetworkPolicy` em `forja-contracts`) consumido por `forja-counter-kit`, `review`, `voice` e `browser`.
- Publica `network.check` para avaliar bloqueio offline e bypass de proxy por URL/host.

## Como testar

```sh
cargo test -p forja-module-network
```
