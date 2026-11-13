+++
title = { pt-BR = "Gerar mensagem de commit", en = "Generate commit message" }
description = "Produz uma mensagem de commit no padrão Conventional Commits a partir do diff."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff"]
arguments = [ { name = "prefix", required = false, default = "" } ]
+++
Analise as alterações do diff da bancada e proponha uma mensagem de commit de alta qualidade.

Padrão esperado:
- **Linha de cabeçalho:** `<tipo>(<escopo>): <descrição curta>` (máximo 72 caracteres, em inglês, imperativo, sem ponto final).
  Prefix sugerido: {{prefix}}
  Tipos permitidos: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.
- **Corpo (opcional mas recomendado):** Separado por uma linha em branco, explicando a motivação da mudança e o que foi realizado, em bullets concisos.
- **Rodapé:** Menção a breaking changes (`BREAKING CHANGE:`) ou tickets relacionados (`Closes #123`), se aplicável.
