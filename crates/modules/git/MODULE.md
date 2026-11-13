# forja-module-git

Módulo `git`: painel Mudanças, histórico com grafo, blame e sincronização.

## O que registra

- Comandos/RPC `git.status`, `git.changes`, `git.refresh`, `git.diff`,
  `git.stage`, `git.stage_all`, `git.unstage`, `git.unstage_all`,
  `git.discard`, `git.commit`, `git.push`, `git.pull`, `git.fetch`,
  `git.sync`, `git.publish`, `git.rebase`, `git.abort`, `git.conflicts`,
  `git.log` e `git.blame`.
- Evento de contrato `git.status_changed` (`bench_path`, `summary`).
- View `git.changes` no painel direito (ordem 5, para ficar visível enquanto o
  shell empilha as views) com as abas Mudanças, Histórico e Blame.
- View `git.history` como aba de bancada (ordem 60) com o grafo e o blame em
  área larga.
- As configurações do módulo `git` do catálogo (identidade, assinatura, sync,
  upstream, branches protegidas e intervalo de status).

O diff textual não é desenhado aqui: `git.diff` devolve os hunks em JSON para o
módulo `diff` da onda C2.

## Comportamento

- **Status:** `git status --porcelain=v2 --branch` é lido periodicamente em
  `Ctx::spawn` e vira `git.status_changed`. O painel também lê `git.changes`
  para a lista por arquivo. Leituras curtas respondem na hora.
- **Escritas e rede:** stage, unstage, discard, commit, push, pull, fetch, sync,
  publish, rebase e abort são despachados por `Ctx::spawn` e devolvem
  `{pending: true}`. O painel mostra `Trabalhando…` e atualiza quando o efeito
  termina.
- **Descartar** exige `confirm: true`; arquivos não rastreados são removidos e
  os rastreados voltam para `HEAD`.
- **Commit** usa `git.identity.*`, `git.commit.sign`, `git.commit.gpg_key` e
  `git.commit.sign_off`. Branches em `git.protected.branches` recusam commit e
  rebase.
- **Push** usa `git.push.auto_set_upstream`, aceita `--force-with-lease` e
  publica o upstream.
- **Fetch automático:** `git.sync.fetch_interval_min > 0` faz fetch em segundo
  plano; `git.sync.auto_pull` puxa quando `always` ou quando a árvore está
  limpa (`clean_only`).
- **Histórico:** `git log --graph --pretty=format:…` é devolvido em linhas com
  o prefixo ASCII do grafo e o commit opcional. A view pinta o grafo em
  monoespaçado, seleciona um commit e mostra autor, e-mail e horário.
- **Blame:** `git blame --line-porcelain` vira linhas com hash, autor, horário
  e conteúdo. O campo aceita um caminho relativo à bancada.
- **Conflitos:** `git.conflicts` lista `--diff-filter=U`; o painel mostra os
  arquivos com cor de perigo.

## Segurança e leituras

Todo comando roda com `GIT_TERMINAL_PROMPT=0`, stdin nulo e um lock de processo
que serializa as chamadas (status também escreve o índice e colide com stage).
Testes e screenshots usam repositórios e pastas temporárias.

## Evidência e hooks de screenshot

- `FORJA_GIT_PANEL=changes|history|blame` seleciona a aba inicial do painel
  direito para os screenshots.
- `FORJA_GIT_BLAME=<arquivo>` carrega o blame desse arquivo ao abrir a aba.

```sh
cargo test -p forja-module-git
cargo run -p forja-app -- --screenshot shots/mudancas.png --slot right
```

Os testes cobrem status, stage/commit/push para um remoto bare local, grafo,
blame e hunks de diff.
