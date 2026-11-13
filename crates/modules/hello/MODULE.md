# forja-module-hello

Módulo `hello` da Forja. Gerado por `cargo xtask new-module hello`.

## O que registra

- comando `hello.greet` (Bigorna e CLI);
- aba `hello.tab` na área de bancada;
- configuração `hello.greeting.style` (`warm` ou `plain`);
- atalho `Ctrl+Alt+H` no escopo `hello` (`keymap.toml`);
- método RPC `hello.greet` com o parâmetro `name` (alias `nome`);
- bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

- Toda chave i18n usada em runtime é anunciada com `Registrar::uses_i18n`.
- O teste do crate roda o contrato do kernel sobre o módulo.
- Nada fora de `crates/modules/hello` precisa ser editado; o vínculo com o
  shell é feito pelo `xtask` na região `# forja:modules`.

## Como testar

```sh
cargo test -p forja-module-hello
cargo run -p forja-app
forja hello greet --nome Ana
```
