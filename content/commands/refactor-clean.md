+++
title = { pt-BR = "Refatorar código para clareza", en = "Refactor for clarity" }
description = "Simplifica e organiza o código sem alterar seu comportamento externo."
scope = ["anvil", "chat"]
run = "helper"
permissions = "accept-edits"
context = ["diff"]
arguments = [ { name = "objective", required = false, default = "legibilidade e manutenibilidade" } ]
+++
Refatore o código do diff atual com foco em {{objective}}.

Diretrizes:
1. Elimine duplicações de código (DRY) e simplifique lógica condicional complexa.
2. Quebre funções excessivamente longas em métodos menores e com responsabilidade única.
3. Melhore a nomenclatura de variáveis, constantes e parâmetros para expressar a intenção claramente.
4. Mantenha estritamente o comportamento externo e a compatibilidade da API pública.
5. Execute os testes existentes para garantir que nenhuma regressão foi introduzida.
