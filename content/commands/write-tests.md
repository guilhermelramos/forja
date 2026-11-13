+++
title = { pt-BR = "Escrever testes para o arquivo", en = "Write tests for file" }
description = "Gera testes unitários abrangentes para o arquivo especificado."
scope = ["anvil", "chat"]
run = "helper"
permissions = "accept-edits"
context = ["bench"]
arguments = [ { name = "file_path", required = true, default = "" } ]
+++
Analise o arquivo {{file_path}} e elabore uma suíte completa de testes automatizados.

Requisitos:
1. Cubra o fluxo principal (caminho feliz) e casos de borda relevantes (entradas nulas/vazias, limites numéricos, timeouts).
2. Teste cenários de falha esperados e validação de erros.
3. Utilize os frameworks e convenções já adotados no projeto (e.g. testes integrados ao módulo ou em diretório de testes padrão).
4. Assegure que os testes sejam herméticos, determinísticos e não dependam de recursos externos não isolados.
