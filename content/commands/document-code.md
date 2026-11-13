+++
title = { pt-BR = "Adicionar documentação de código", en = "Document code" }
description = "Gera documentação técnica e comentários explicativos para símbolos públicos."
scope = ["anvil", "chat"]
run = "helper"
permissions = "accept-edits"
context = ["bench"]
arguments = [ { name = "target_symbol", required = false, default = "funções e tipos públicos" } ]
+++
Analise o arquivo ou módulo em foco e adicione documentação técnica para {{target_symbol}}.

Diretrizes:
1. Siga o formato de documentação padrão da linguagem (e.g. rustdoc `///`, JSDoc `/** */`, docstrings Python `"""`).
2. Descreva de forma objetiva o propósito do tipo ou função, invariantes e pré/pós-condições.
3. Documente cada parâmetro, tipo de retorno e cenários de erro possíveis.
4. Adicione um exemplo de uso mínimo e correto quando oportuno.
5. Evite redundâncias e descrições óbvias que apenas repetem o nome do identificador.
