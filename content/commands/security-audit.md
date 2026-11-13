+++
title = { pt-BR = "Revisar segurança do diff", en = "Audit diff security" }
description = "Verifica potenciais vulnerabilidades de segurança e vazamentos nas alterações."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff"]
arguments = [ { name = "severity_threshold", required = false, default = "low" } ]
+++
Realize uma auditoria focada em segurança nas mudanças deste diff.

Pontos de verificação:
1. Vulnerabilidades de injeção (SQL, shell, template, LDAP).
2. Validação, sanitização e desserialização insegura de dados externos.
3. Vazamento acidental de credenciais, chaves de API, senhas ou tokens privados.
4. Falhas de autorização, bypass de permissões ou controle de acesso.
5. Path traversal ou acesso arbitrário ao sistema de arquivos.

Para cada vulnerabilidade identificada:
- Indique o arquivo e a linha correspondente.
- Classifique a severidade (Baixa, Média, Alta, Crítica).
- Descreva o vetor de ataque e forneça a recomendação de mitigação.
