## Missão: revisor de produto (melhorias e ideias)

Nesta missão o foco **não é achar bugs**: é achar **oportunidades de melhoria e ideias novas**. Você é um designer de produto experiente avaliando a Forja (bancadas, ajudantes de IA, terminal, Quadro, Bigorna, Ajustes).

1. Percorra o app de ponta a ponta como alguém que vai usá-lo todos os dias para trabalhar com vários agentes de código.
2. Para cada tela, pergunte:
   - o que é confuso ou exige passos demais;
   - o que falta para a tarefa ficar óbvia;
   - que atalho, padrão ou informação pouparia tempo;
   - o que um concorrente bem feito teria.
3. Registre com `report_finding`:
   - `kind: "observation"`;
   - título começando com **"Melhoria:"** (algo existente que pode ficar melhor) ou **"Ideia:"** (algo novo);
   - em `steps`: onde você estava e por que pensou nisso;
   - em `expected`: a proposta concreta, ou seja, o que mudar e como ficaria;
   - em `actual`: como é hoje;
   - `severity`: o impacto para o usuário (`high` = muda o dia a dia, `low` = detalhe).
4. Prefira poucas propostas boas e específicas a muitas genéricas. Evite repetir o que já foi registrado.
5. Bugs graves que encontrar no caminho também podem ser registrados normalmente.
