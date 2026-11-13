# forja-module-trust

Registro único de aprovações por **hash SHA-256 exato** para conteúdo que
executa código vindo de um projeto ou de uma extensão.

## O que registra

- Comandos `trust.check`, `trust.request`, `trust.revoke`, `trust.list`.
- RPC dos mesmos nomes; `trust.request` devolve `request_id` e a decisão chega
  pelo evento `trust.decided`.
- Evento de contrato `trust.decided` (`TrustDecided`).
- Um modal de aprovação no slot de barra de status: aprovar uma vez (só nesta
  sessão), aprovar este conteúdo (persistente) ou recusar.
- Seção de Ajustes com todas as decisões e botão de revogar.

## Regras

- A decisão vale para `(kind, source_path, content_hash)`. Qualquer byte novo
  muda o hash e a pergunta volta.
- `kind`: `setup`, `teardown`, `recipe`, `command`, `extension`,
  `helper_profile`.
- Persistência em `%APPDATA%\Forja\modules\trust.json`, em segundo plano.
- Aprovação "uma vez" não é gravada e desaparece ao fechar o app.
- Recusa é gravada com `approved = false`; `trust.check` devolve
  `decided_at`, e o conteúdo continua bloqueado.

## Migração dos outros módulos

`c1-benches-git` (scripts do `forja.toml`) e `terminal` (receitas de projeto)
ainda aprovam localmente. A API está pronta: trocar por `trust.check` e
`trust.request` com o hash do conteúdo; os dois recebem `approved` no retorno e
`trust.decided` como evento assíncrono. Nenhum arquivo desses módulos foi
editado por esta tarefa.

## Como testar

```sh
cargo test -p forja-module-trust
cargo xtask provenance
```
