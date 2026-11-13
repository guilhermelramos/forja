+++
title = { pt-BR = "Explicar erro do terminal", en = "Explain terminal error" }
description = "Diagnostica uma saída de erro de compilação ou execução e propõe correções."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["bench"]
arguments = [ { name = "error_output", required = false, default = "saída recente do terminal" } ]
+++
Analise a seguinte mensagem ou log de erro capturado no terminal:

```
{{error_output}}
```

Tarefas:
1. Explique a causa raiz do erro em linguagem clara e direta.
2. Indique exatamente onde o problema se originou (arquivo, módulo, função ou dependência).
3. Apresente os passos ou o código específico necessário para corrigir o erro.
4. Se houver mais de uma causa possível, liste-as por ordem de probabilidade.
