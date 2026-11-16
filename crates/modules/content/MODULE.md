# Módulo `content`

Conteúdo da Forja que é **arquivo**, não código compilado. O módulo descobre as
três camadas, valida cada arquivo, mantém o registro em memória e recarrega com
`notify` quando `content.reload.watch` está ligado.

## Camadas

| Camada | Raiz |
|---|---|
| embutida | `content/` do repositório (ou `content/` ao lado do executável) |
| usuária | `~/.forja/` (ou a pasta `user_dir` do runtime) |
| projeto | `<bancada>/.forja/` |

A camada de projeto substitui a usuária, que substitui a embutida. Para o mesmo
tipo, a chave é o id/nome; empate dentro da mesma camada é resolvido por ordem
alfabética do arquivo e vira diagnóstico.

## O que registra

- **Comandos** (`commands/*.md`): frontmatter TOML entre `+++`, corpo com
  `{{placeholders}}`. Viram linhas da Bigorna e do `/comandos` do chat pelo
  contrato `content.commands.list`. `content.run` executa.
- **Temas** (`themes/*.toml`): o terminal consulta `content.themes.list`; a
  interface consulta `content.themes.ui`.
- **Modelos** (`templates/*.md`): `content.templates.list`, `get` e `render`.
  O painel Arquivos oferece os modelos no diálogo "Novo arquivo".
- **Receitas** (`recipes/*.toml`): `content.recipes.list`; o terminal usa essa
  lista e cai para o leitor local se o módulo não estiver presente.

## Trust

Comando de projeto com `run = "terminal"` ou `permissions` `automatic`/`full`
só executa depois de `trust.check` aprovado para o hash exato do arquivo. Sem
aprovação ele aparece desabilitado e `content.run` responde `[trust]`.

## Testar

```sh
cargo test -p forja-module-content
```

Na Bigorna, as categorias são traduzidas e os atalhos declarados acompanham o comando; diagnósticos mostram caminho relativo e mensagem legível, com detalhes recolhidos.
