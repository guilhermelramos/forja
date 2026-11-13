+++
title = { pt-BR = "Resumir as mudanças da bancada", en = "Summarize bench changes" }
description = "Gera um resumo executivo de todas as alterações feitas na bancada atual."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff", "bench"]
arguments = [ { name = "detail_level", required = false, default = "standard" } ]
+++
Por favor, analise as diferenças (diff) desta bancada e forneça um resumo executivo claro e conciso.

Nível de detalhe: {{detail_level}}

Diretrizes:
1. Liste os principais módulos e arquivos modificados.
2. Agrupe as mudanças por finalidade: novas funcionalidades, correções de bugs, refatoração ou testes.
3. Destaque qualquer mudança com potencial impacto ou risco de compatibilidade.
4. Mantenha a resposta objetiva em formato de tópicos com marcadores.
