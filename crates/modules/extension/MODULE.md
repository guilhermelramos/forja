# Módulo `extension`

Runtime Lua das extensões da Forja. Cada extensão é uma pasta com
`extension.toml` e `init.lua` em `~/.forja/extensions/<id>/` ou
`<projeto>/.forja/extensions/<id>/`.

## Ciclo

1. Descoberta das duas raízes.
2. Hash SHA-256 da pasta inteira (caminho, tamanho e conteúdo, em ordem).
3. Estado: `pending`, `active`, `changed`, `invalid`, `suspended`, `conflict`.
4. Aprovação pelo `trust` (kind `extension`) e registro de capacidades em
   `<dados>/approvals/extensions.json`.
5. Estado Lua novo em uma thread própria, `init.lua`, registro estático de
   comandos, painéis e configurações.
6. Eventos e comandos entram numa fila; a interface recebe árvores de painel.

## Sandbox

Bibliotecas: `coroutine`, `table`, `string`, `utf8`, `math`. `os`, `io`,
`package`, `require`, `debug`, `dofile`, `loadfile` e `string.dump` são
removidos. `load` só aceita texto. Memória e tempo de CPU têm limites; um laço
infinito é interrompido pelo hook de instruções.

## API

`forja.commands.register`, `forja.on`, `forja.ui.*`, `forja.settings.*`,
`forja.terminal.*`, `forja.git.*`, `forja.fs.*`, `forja.http.*`, `forja.notify`,
`forja.i18n`, `forja.log`, `forja.store`, `forja.time`, `forja.project`,
`forja.bench` e `forja.load`.

## Página e painéis

- `extension.page` no trilho: estado, capacidades, aprovar/revogar, recarregar,
  configurações, comandos, erros e log.
- `extension.panels.*`: desenham a árvore declarativa em `right_panel`,
  `bench_tab` e `sidebar`.
- `extension.commands` na Bigorna.

## Testar

```sh
cargo test -p forja-module-extension
```
