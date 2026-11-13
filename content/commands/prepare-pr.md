+++
title = { pt-BR = "Preparar descrição de PR", en = "Prepare PR description" }
description = "Gera título e descrição detalhada para abertura de Pull Request."
scope = ["anvil", "chat"]
run = "helper"
permissions = "ask"
context = ["diff", "request", "bench"]
arguments = [ { name = "template_type", required = false, default = "standard" } ]
+++
Com base nas alterações da bancada e no contexto do pedido ({{request.title}}), prepare uma proposta de título e descrição para abertura de Pull Request (ou Merge Request).

Formato esperado:
- **Título:** Claro, conciso, no padrão Conventional Commits (e.g. `feat(escopo): descrição`).
- **Resumo das Mudanças:** O que foi alterado e qual a motivação técnica/negócio.
- **Decisões Técnicas:** Principais escolhas de arquitetura ou bibliotecas, se aplicável.
- **Como Testar:** Instruções passo a passo para reproduzir a validação localmente.
- **Impactos:** Relação de migrações, alterações de configuração ou eventuais quebras de compatibilidade.
