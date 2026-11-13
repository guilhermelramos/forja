# Forja: Livro do Produto

> **Fonte de verdade do produto.** Os conceitos, nomes e fluxos que estão aqui são nossos. Toda funcionalidade nova começa por este documento. Ele nunca é derivado de outro produto.

## 1. A ideia

A **Forja** é a oficina do desenvolvedor que trabalha **com agentes de código**. Num único lugar você abre bancadas isoladas para cada linha de trabalho, põe agentes para trabalhar nelas, acompanha o que cada um está fazendo, revisa o resultado e entrega.

A metáfora é a forja de um ferreiro:
- **Projeto:** a peça que está sendo trabalhada.
- **Bancada:** o lugar onde cada parte da peça é trabalhada.
- **Agentes:** os ajudantes que trabalham nas bancadas.
- **Mutirão:** quando muitos ajudantes trabalham juntos, coordenados.
- **Bigorna:** onde tudo se faz rápido, ou seja, a paleta de comandos.

Princípios:
1. **Tudo é módulo.** O app é um núcleo pequeno, e cada funcionalidade é um módulo que se registra sozinho.
2. **Tudo é arquivo.** Comandos, perfis de agentes, skills, temas, atalhos e rotinas são arquivos de texto que você ou um modelo editam, sem recompilar.
3. **Modelos são cidadãos de primeira classe.** Agentes operam a Forja pelo CLI `forja` e por ferramentas MCP, e conseguem estendê-la criando uma pasta.
4. **Nada escondido.** A Forja nunca altera arquivos fora das pastas dela (configs de outros apps, por exemplo) sem um "sim" explícito e registrado.
5. **Local primeiro.** Tudo funciona offline. As integrações usam as suas contas e ferramentas, com segredos no cofre do sistema.

## 2. Vocabulário (UI pt-BR / UI en / id no código)

| Conceito | pt-BR | en | id | O que é |
|---|---|---|---|---|
| Repositório ou pasta registrada | Projeto | Project | `project` | Uma pasta (git ou não) que a Forja conhece |
| Linha de trabalho isolada | **Bancada** | Bench | `bench` | Um checkout próprio (worktree git) com branch, terminais, agentes e notas. A bancada principal é o checkout original. |
| Agente CLI em execução | **Ajudante** | Helper | `helper` | Uma instância de um agente (Claude Code, Codex, OpenCode…) rodando numa bancada |
| Definição de um agente | Perfil de ajudante | Helper profile | `profile` | Arquivo TOML que diz como detectar, iniciar, passar prompt e permissões de um CLI de agente |
| Conversa de um ajudante | Sessão | Session | `session` | O histórico de uma execução de ajudante (TUI ou chat) |
| Trabalho a fazer | **Pedido** | Request | `request` | Issue/PR/ticket de qualquer fonte, ou um pedido local |
| Fonte de pedidos | Balcão | Counter | `counter` | GitHub, GitLab, Linear, Jira ou local |
| Estado de uma bancada | Etapa | Stage | `stage` | `rascunho` → `forjando` → `revisão` → `pronto` (customizável) |
| Visão por etapas | Quadro | Board | `board` | Colunas por etapa com as bancadas como cartões |
| Ação agendada | **Rotina** | Routine | `routine` | Prompt para um ajudante ou comando, num horário (cron) e num alvo |
| Coordenação multiajudante | **Mutirão** | Muster | `muster` | Um coordenador divide o trabalho em tarefas e despacha para ajudantes em bancadas |
| Unidade de trabalho do mutirão | Tarefa | Task | `task` | Uma parte do mutirão, com spec, dependências e estado |
| Mensagem do mutirão | Recado | Note | `note` | Comunicação coordenador ↔ ajudante: `status`, `pergunta`, `resposta`, `entrega`, `alerta` |
| Entrega de uma tarefa | Entrega | Handoff | `handoff` | O ajudante declara o resultado: `feito`, `falhou` ou `bloqueado`, com evidências |
| Paleta de comandos | **Bigorna** | Anvil | `anvil` | Ctrl+K: busca tudo e executa qualquer comando |
| Atalho de busca de arquivos | Achar arquivo | Find file | `find` | Ctrl+P |
| Comando curto salvo | Receita | Recipe | `recipe` | Comando de terminal com nome, escopo (global/projeto) e Enter opcional |
| Habilidade de agente | Skill | Skill | `skill` | Pasta com `SKILL.md` (formato aberto do ecossistema de agentes) |
| Pacote de extensão | Extensão | Extension | `extension` | Pasta com `extension.toml` + Lua e/ou arquivos de conteúdo |
| Sessões antigas indexadas | Acervo | Archive | `archive` | Busca e retomada de sessões passadas de todos os ajudantes |
| Arquivo de config por repo | `forja.toml` | `forja.toml` | `repo_config` | Scripts de preparo e desmonte de bancada, abas iniciais e comandos de pedido |

Regras de nomenclatura no código:
- Ids em **inglês**, minúsculos e com pontos: `bench.create`, `helper.launch`, `terminal.split.right`.
- Chaves de configuração seguem o padrão `<módulo>.<grupo>.<nome>` em snake_case: `terminal.scroll.history_lines`.
- Textos de UI vêm sempre do i18n (Fluent), em pt-BR e en.

## 3. Arquitetura de informação (o que o usuário vê)

```
┌ Barra superior: projeto ▸ bancada · Achar arquivo (Ctrl+P) · Bigorna (Ctrl+K) · tema · janela
├ Trilho (esquerda): Bancadas · Quadro · Pedidos · Ajudantes · Mutirão · Rotinas · Acervo · Extensões · Ajustes
├ Lateral: lista de projetos e bancadas (agrupar, ordenar, filtrar, fixar)
├ Área central: abas da bancada — Terminal · Ajudante (TUI ou Chat) · Editor · Diferenças · Navegador · Revisão (PR)
├ Painel direito (recolhível): Arquivos · Mudanças · Busca · Checks · Portas
└ Barra de status: branch e sincronização · ajudantes ativos · uso · portas · recursos · notificações
```

Cada item do trilho, cada aba, cada painel e cada seção de Ajustes é **contribuído por um módulo** (slot). O shell não sabe quais existem.

## 4. Fluxos principais (redesenhados)

### 4.1 Do pedido à entrega
1. Em **Pedidos** você escolhe um pedido (ou cria um local) e clica em **Forjar**.
2. A Forja cria uma **bancada**:
   - nome e branch são gerados a partir do pedido (`pedido-123-titulo-curto`), com o prefixo configurável;
   - roda o preparo do `forja.toml`, se aprovado;
   - abre as abas iniciais.
3. Um **ajudante** (o padrão do projeto) inicia já com o contexto do pedido no prompt.
4. O status do ajudante (trabalhando, precisa de você, terminou) aparece na bancada, no Quadro e numa notificação.
5. Em **Diferenças** você revisa, deixa notas por linha e manda as notas de volta ao ajudante.
6. **Revisão** abre o PR/MR com título e descrição gerados pelo ajudante, acompanha os checks e permite "consertar com ajudante".
7. Ao entregar, a bancada passa para a etapa `pronto` e pode ser **guardada**: o desmonte roda, os terminais fecham e a branch pode ser mantida ou apagada.

### 4.2 Mutirão (coordenação)
- Você (ou um ajudante coordenador) escreve um objetivo. A Forja propõe **tarefas** com dependências, que você ajusta.
- Cada tarefa é despachada a um ajudante, na bancada atual ou numa bancada nova.
- Os ajudantes mandam **recados** (`forja mutirao recado …`) e encerram com uma **entrega** (`forja mutirao entregar --resultado feito …`).
- O coordenador recebe tudo numa caixa de entrada. Nunca digitamos num ajudante que está pedindo aprovação: o recado espera, e o coordenador é avisado.

### 4.3 Rotinas
- Uma rotina tem um horário (presets ou cron), um alvo (bancada fixa ou nova), uma ação (prompt para ajudante ou comando) e uma pré-checagem opcional.
- Ela guarda um histórico com a saída resumida e notifica quando falha.

### 4.4 Estender a Forja (para pessoas e modelos)
- **Sem compilar:** criar arquivos em `.forja/` do projeto ou em `~/.forja/`:
  - `commands/*.md` vira um comando na Bigorna e um `/comando` no chat;
  - `helpers/*.toml` define um perfil novo de agente;
  - `skills/*/SKILL.md`, `themes/*.toml`, `keymaps/*.toml` (e `keymap.toml` para os ajustes do usuário), `recipes/*.toml`, `templates/*.md`.
- **Com Lua:** `extensions/<nome>/extension.toml` + `init.lua` registram comandos, reagem a eventos, criam painéis declarativos e configurações. As capacidades pedidas são aprovadas uma vez.
- **Com Rust:** `cargo xtask new-module <nome>` cria um crate em `crates/modules/<nome>` que se registra sozinho.

## 5. O que a Forja não terá (por decisão)
- Contas ou nuvem próprias (links públicos, perfis em nuvem, servidores remotos da Forja).
- Pareamento com celular, emuladores e VMs efêmeras.
- Execução de código de extensão sem sandbox (as extensões Lua só chegam ao que pediram e você aprovou).
- Telemetria.

## 6. Identidade
Ficam em `design/brand.md`: marca, logo (bigorna/centelha), paleta "brasa", tipografia e tom de voz.
