# Item 12 — apresentação para quem usa a Forja

Issue: #201. Integração da main: `d102908`, preservando a recuperação de pânico e o contador de quadros; a regressão de pânico passou de 46 para 47 porque a main acrescentou a regressão 46 de repouso.

## Entrega

| Linha da issue | Resultado |
| --- | --- |
| Barra de status | Saíram “Pronto” e a contagem de módulos; bancada e branch vêm do módulo de bancadas, contadores de trabalho/espera vêm do módulo de ajudantes, uso diário continua no módulo de uso e notificações mostram sino e contagem inclusive zero. Tudo contribui pelo slot StatusBar, sem importação dos módulos pelo shell. A consulta de branch usa Tasks e o wrapper de processo existente. |
| Bigorna | Comandos de conteúdo mostram categoria traduzida e atalho declarado, quando existe. As etiquetas também são acessíveis, permitindo detectar regressões por snapshot. Identificadores continuam disponíveis para busca interna. |
| Extensões | Hash, origem e caminho estão dentro de “Detalhes”, inicialmente recolhido. |
| Chat: modelo | Seletor “Modelo”, opções declaradas no perfil e campo “Outro modelo” dentro do seletor preservam nomes personalizados sem expor variáveis de ambiente. |
| Chat: contexto | Nome do ajudante e pasta final (`…\forja`), sem id da sessão nem separador vazio. |
| Conteúdo | Cabeçalhos ausentes ou incompletos têm orientação traduzida; demais erros têm resumo traduzido e diagnóstico técnico recolhido. Caminhos são relativos ao projeto/usuário, ou mostram somente o nome do arquivo quando pertencem a outra raiz; segmentos de pais são normalizados. |
| Notificações | O Fluent integrado da main já formatava corretamente números; teste adicional usa os bundles reais e confere “1 aviso” e “2 avisos”. O sino agora inclui a contagem zero. |
| Janela | Botões da barra têm nomes acessíveis traduzidos: “Fechar janela”, “Minimizar” e “Maximizar”. |

Outros vazamentos corrigidos: revisão interna `rev N` no cabeçalho de Conteúdo, camada `builtin/user/project` nos diagnósticos, ids de sessão no chat e valores de esforço em inglês (baixo/médio/alto/muito alto/máximo agora traduzidos).

## Prova ao vivo

Todos os processos foram iniciados com `--test-driver`, fora da tela, `FORJA_TEST_HELPERS=none`, pipe e dados próprios. Uso previsível nas capturas finais: `FORJA_USAGE_FIXTURE=1`. Nenhum ajudante foi iniciado.

Artefatos ficam em `%TEMP%\forja-e12-evidence`, preservados conforme instrução do coordenador:

- `baseline-chat.png` e `baseline-anvil.png`: antes, binário recompilado com a árvore anterior às alterações desta issue; mostram `ANTHROPIC_MODEL`, contexto completo, “Pronto”, “56 módulos” e quatro etiquetas visíveis `builtin · helper`.
- `final-chat.png` e `final-anvil.png`: depois; seletor Modelo, pasta abreviada e quatro etiquetas “Ajudante”.
- `before-content.txt` / `after-content.txt`: diagnósticos antes/depois, incluindo os arquivos embutidos com cabeçalho ausente.
- `before-main-extensions.txt` / `after-main-extensions.txt`: extensão de exemplo instalada somente na pasta isolada; hash visível antes, recolhido depois.
- `before-main-notifications.txt` / `after-main-notifications.txt`: aviso isolado, plural e sino.
- `drive-after.txt`: saída de drive.ps1 com chat, adicionar projeto, abrir Bigorna e pesquisar “revisar”.
- `drive-status.txt`: adicionar projeto e aguardar a consulta assíncrona; contexto de bancada, branch, contadores, sino e uso diário.

Em 1280×800 lógico (1920×1200 físico nesta máquina), os itens da barra ficaram dentro de x=0..1920 e y=1154..1200; o último item terminou em x=1893, y=1188. A bancada tem largura limitada e dica com o texto completo para nomes extensos.

## Testes e regressões

- Testes unitários: contexto bancada/branch, contagem de estados dos ajudantes, caminhos relativos de diagnósticos, pasta abreviada do chat, escolhas de modelo declaradas no perfil e plural real de notificações.
- Regressão 48: ausência de “módulos”/“builtin” na tela principal e na Bigorna, com nomes acessíveis da janela e presença da própria Bigorna.
- Regressão 49: chat sem ajudante real, seletor Modelo, pasta abreviada e ausência de variável de ambiente/id de prévia.
- Regressão 01 ajustada: o botão principal agora também se chama “Fechar janela”; fechamento de Ajustes continua conferido por zero janelas e desaparecimento do conteúdo do diálogo.

Validação: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo xtask provenance` e `cargo xtask catalog` passaram. Replay completo: **54/54 em 1280×800 e 54/54 em 1920×1080**; depois da correção final que agenda a consulta de branch pela view de status, passaram novamente os testes completos e as regressões 46/48/49 em ambos os tamanhos. A prova de branch após a correção mostrou `main · forja-e12-example`.

Logs de validação: `%TEMP%\forja-e12-tests.log`, `forja-e12-clippy.log`, `forja-e12-replay-small.log` e `forja-e12-replay-large.log`. Proveniência final: 985 arquivos, 524 hashes; catálogo: 273 configurações registradas, 272 com efeito e 110/110 atalhos (mesmos totais da integração).

## Limites

Os diagnósticos técnicos continuam disponíveis sob expansão explícita; nomes personalizados de modelo e valores adicionais de esforço declarados por perfis próprios são preservados. Nenhum módulo novo nem dependência foi acrescentado; não houve alteração no laço principal, em paint_view ou no logger nesta issue. Pastas temporárias permanecem para o coordenador limpar, conforme solicitado.
