# forja-kernel-macros

Macro `#[forja_kernel::module]`. Recebe um `static` cuja inicialização é uma
função `fn() -> Box<dyn Module>` e:

1. prepende `#[linkme::distributed_slice(::forja_kernel::MODULE_FACTORIES)]`;
2. exporta `__forja_module()` chamando o static, usado pelo fallback de
   linking do shell.

Não depende de `syn`/`quote`: a única informação extraída é o identificador
depois de `static`. Um módulo por crate (o nome `__forja_module` é fixo).

## Como testar

Compilado e exercitado pelo `forja-module-hello`:

```sh
cargo test -p forja-module-hello
```
