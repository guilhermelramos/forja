# forja-module-projects

Módulo `project` (pasta `projects`): as pastas que a Forja conhece.

## O que registra

- Comandos e RPC `project.list`, `project.add`, `project.remove`, `project.create`,
  `project.clone` e `project.update`; `project.add_dialog` abre o diálogo de
  adicionar pasta a partir de qualquer tela.
- Eventos de contrato `project.added` e `project.removed`.
- A view `project.sidebar` no slot `sidebar` (ordem 10): projetos com ícone, cor e
  a lista de bancadas de cada um.
- A view `project.dialogs` no slot `modal` (ordem 20): os diálogos de projeto.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

Não há configurações próprias: o catálogo não define um grupo de Ajustes para
projetos. O arquivo do módulo fica em `<dados>/modules/project.json`.

## Desenho da lateral

1. `project.sidebar` desenha os projetos. Ao clicar num projeto, ele chama
   `bench.list_cards {project_id}`, ativa a bancada principal e expande a lista de
   bancadas; ao expandir, `bench.list {project_id}` mostra as bancadas devolvidas.
2. O módulo `board` contribui com uma segunda view de lateral (`board.sidebar`,
   ordem 20) com filtro, agrupamento, ordenação, fixar e lido/não lido.
3. Se o módulo de bancadas não estiver carregado, `bench.list` falha, a lateral
   mostra uma dica e o resto do módulo continua funcionando.

## Regras

- `project.add` canonicaliza a pasta, recusa duplicata e não altera arquivos.
- `project.remove` só remove a entrada; a pasta e o repositório ficam no disco.
- `project.create` cria uma pasta vazia e roda `git init`; `project.clone` roda
  `git clone`. Os dois são assíncronos e devolvem `{pending: true}`.
- Todos os diálogos (Adicionar pasta, Novo projeto, Clonar, Editar e Remover)
  registram a camada de teclado: o Tab fica na janela e Escape fecha.
- Adicionar pasta e Novo projeto focam o campo "Pasta" ao abrir; Enter nesse
  campo confirma o diálogo, então o formulário funciona só com teclado.
- Ícone é um texto curto (emoji ou letra) e cor é `#RRGGBB`.
- O id é estável, derivado do caminho normalizado; re-adicionar o mesmo caminho
  não duplica o projeto.
- Git roda com `GIT_TERMINAL_PROMPT=0` e stdin nulo.

## Como testar

```sh
cargo test -p forja-module-projects
cargo run -p forja-app -- --screenshot shots/lateral.png --slot sidebar
```

Os testes usam pastas temporárias, criam um repositório git local e removem tudo
no final; nenhum teste toca `%APPDATA%\Forja` ou `~/.forja`.
