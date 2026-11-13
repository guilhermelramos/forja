+++
title = { pt-BR = "Revisar as mudanças da bancada", en = "Review bench changes" }
description = "Revisa o diff da bancada buscando bugs, omissões e melhorias."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff", "request", "bench"]
arguments = [ { name = "focus", required = false, default = "all" } ]
+++
Por favor, faça uma revisão minuciosa de código no diff atual da bancada, com foco em {{focus}}.

Contexto do pedido associado: {{request.title}}

Diretrizes da revisão:
1. Identifique possíveis bugs, regressões e tratamentos ausentes de erros ou exceções.
2. Aponte casos de borda sem cobertura de testes.
3. Avalie clareza de nomes, separação de responsabilidades e aderência às convenções do repositório.
4. Para cada problema encontrado, informe o arquivo, número aproximado da linha, explicação objetiva do risco e sugestão de correção.
5. Não aplique edições automáticas; retorne uma lista estruturada de observações.
