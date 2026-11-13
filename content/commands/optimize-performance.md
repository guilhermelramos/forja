+++
title = { pt-BR = "Otimizar desempenho", en = "Optimize performance" }
description = "Identifica gargalos e propõe melhorias de eficiência e consumo de memória."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff", "bench"]
arguments = [ { name = "target", required = false, default = "mudanças recentes" } ]
+++
Analise o código em {{target}} procurando gargalos de desempenho e oportunidades de otimização.

Focos de análise:
1. Complexidade de tempo excessiva (loops aninhados desnecessários, algoritmos O(N^2) ou piores).
2. Alocações de memória frequentes, clonagem redundante de dados ou buffers não reutilizados.
3. Consultas bloqueantes ou falta de paralelismo/concorrência onde apropriado.
4. I/O síncrono ou operações repetitivas em coleções grandes.

Apresente as sugestões com estimativas de impacto e exemplos comparativos antes/depois.
