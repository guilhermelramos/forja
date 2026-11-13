# xtask

Ferramentas do repositório. Não faz parte do produto empacotado.

```sh
cargo xtask new-module <nome>
cargo xtask new-extension <nome>
cargo xtask provenance
cargo xtask package [--skip-build]
```

## new-module
Gera `crates/modules/<nome>` com manifesto, i18n en/pt-BR, `keymap.toml`,
`MODULE.md`, comando + view + setting + RPC de exemplo e teste de contrato.
Acrescenta a dependência apenas na região `# forja:modules` de
`crates/shell/Cargo.toml`. Recusa nome inválido ou pasta existente.

## new-extension
Gera `extensions/<nome>/extension.toml`, `init.lua` e `README.md`. O carregador
Lua e a aprovação de capacidades são da Fase D.

## provenance
Lê `git ls-files`, normaliza para minúsculas palavras, ids com ponto/hífen,
camelCase de 8+ caracteres, literais entre aspas, linhas e frases de 4+ palavras,
calcula SHA-256 e compara com `xtask/provenance-denylist.sha256`. Arquivos em
`xtask/provenance-allow.toml` são ignorados com o motivo registrado. Falha se
houver qualquer hash conhecido. Roda em poucos segundos.

## package
Compila em release (salvo `--skip-build`), monta
`dist/forja-<versão>-win-x64/` com os três executáveis, README, licença,
`docs/modules.md` e conteúdo, e grava um zip **stored** (sem dependência de zip
externo). Imprime tamanho e SHA-256.

## Como testar

```sh
cargo test -p xtask
cargo xtask provenance
```
