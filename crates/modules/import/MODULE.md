# forja-module-import

Importador opcional dos dados do Harbor 0.1 (nosso app anterior). Lê `session.json` e `settings.json` da pasta do Harbor, mostra o que pode vir e importa os itens escolhidos.

## O que importa

| Item | Vai para |
|---|---|
| projetos | `project.add` (ignora pastas já registradas) |
| bancadas (worktrees) | o projeto dono é registrado e o bench reconhece os worktrees que já existem; nada é criado no git |
| quick commands | `~/.forja/recipes/imported.toml`, como receitas de terminal (`global` ou `project`) |
| notas | `~/.forja/imported-notes.md` (nunca sobrescreve um arquivo existente) |
| destinos SSH | `remote.ssh.save` (ignora mesmo host e rótulo já salvos) |
| tema claro/escuro | `appearance.theme.mode` |
| fonte do terminal | `appearance.ui.code_size` (10 a 24) |

Nada além disso. Tokens e segredos do Harbor nunca são lidos: só os campos listados em `src/harbor.rs`. Os segredos continuam no cofre do Windows, e o usuário reconecta os balcões.

## Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `import.harbor.status` | — | `{loaded, busy, available, pending, decided, error, items[], summary}`. Não lê disco. `items[]` traz `id`, `kind`, `label`, `detail` e `imported` |
| `import.harbor.preview` | — | `{started: true}`. Lê o Harbor e o store em segundo plano |
| `import.harbor.run` | `{items?: [id]}` | `{started}`. Sem `items`, importa tudo que está pendente. Resultado em `summary` |
| `import.harbor.skip` | — | `{started}`. Grava a decisão "pulado" |

`pending` vale quando `session.json` existe e ainda não há decisão. O passo de importação do onboarding usa esse valor.

## Seção de Ajustes

`import.settings` (Ajustes, seção de módulos) mostra o estado e o botão "Importar pendentes". Ao abrir, pede `import.harbor.preview`.

## Armazenamento

- `<dados>/modules/import.json`: decisão (`imported` ou `skipped`) e ids já importados. Com isso, executar de novo não duplica nada.
- Gravações fora do store: só em `~/.forja` (receitas e notas). Testes usam pastas temporárias.

## Caminhos

- Harbor: `%APPDATA%\Harbor`, ou `FORJA_HARBOR_DIR` (testes e capturas). Somente leitura.
  Em execução de teste (`--test-driver`, `--screenshot`) ou com dados isolados
  (`FORJA_DATA_DIR`), o Harbor real da máquina não é lido; só `FORJA_HARBOR_DIR`
  fornece uma origem nesse caso.
- Conteúdo: `FORJA_CONTENT_DIR` ou `~/.forja`.

## Contratos

- Consome: `project.list`, `project.add`, `remote.ssh.list`, `remote.ssh.save` e `set_setting`. Se um comando não existe, o erro vai para o resumo e nada quebra.
- Publica: nenhum evento.
- Onboarding (`onboarding`): o passo "Importar do Harbor" chama os comandos acima. Esta página foi acrescentada ao onboarding com aprovação do coordenador.

## Como testar

```sh
cargo test -p forja-module-import
```

Fixtures em `tests/fixtures/harbor/` (dados inventados, no formato do Harbor). Os testes cobrem o mapeamento de cada item, arquivo corrompido, nada escrito fora de `user_dir` e o Harbor intacto, executar duas vezes sem duplicar, e a resolução da origem: `FORJA_HARBOR_DIR` vence, e uma execução isolada nunca cai em `%APPDATA%\Harbor`.
