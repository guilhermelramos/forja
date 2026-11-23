# Terminal: roteiro manual de 30 minutos

Este roteiro é para o dono executar, com ferramentas que já instalou e autorizou; os testes automatizados usam somente shells comuns e nunca iniciam agentes reais. Use um projeto descartável, sem segredos, e mantenha um segundo terminal externo disponível para recuperar um processo que travar. Não é necessário instalar nenhuma ferramenta para concluir a parte automatizada.

## Preparar — minutos 0 a 3

Abra uma bancada de teste e o Terminal da Forja; anote versão/commit, Windows, escala da tela e tamanho da janela. Comece em 1280×800 e depois maximize. Prepare um arquivo com 200 linhas numeradas, copie seu conteúdo e mantenha uma alteração descartável para `git diff --color=always` e commits para `git log --color=always`. Confira acentos, CJK e emoji: `ação 中 😀 ❤️`.

## Agentes — minutos 3 a 19

Reserve quatro minutos para cada ferramenta: Claude Code, Codex, OpenCode e Aider. Abra cada uma pelo ajudante configurado na Forja, com suas opções usuais; não aprove comandos ou alterações que não entenda. Se uma ferramenta estiver indisponível, registre “não executada”, a razão e use o tempo para repetir uma das disponíveis.

Em cada agente, siga esta sequência:

1. Abra e confira se a tela inicial ocupa a grade inteira, sem restos do prompt anterior; registre o formato e o piscar do cursor.
2. Peça uma explicação longa sobre arquivos do projeto, sem modificar nada. Durante a resposta, role para cima e volte ao fim: não deve haver saltos, caracteres duplicados, linhas antigas atravessando as novas nem tela parcialmente apagada.
3. No meio da resposta, altere a largura e a altura da janela e divida o painel. Confira caracteres largos nas bordas, o cabeçalho fixo, a posição do cursor e a volta ao acompanhamento da saída.
4. Cole as 200 linhas no campo de entrada. Confira primeira/última linha e quebras; a colagem não deve enviar cada linha como um pedido separado. Cancele antes de enviar se não quiser consumir tokens.
5. Use roda e clique nos controles que a ferramenta oferece. Troque o foco para outro painel e para outra janela, depois volte: o programa deve atualizar seu foco; o cursor sem foco fica parado e vazado.
6. Cancele a resposta e saia pelo comando normal da ferramenta. O shell deve reaparecer legível, com cores, rolagem e cursor recuperados. Se precisar interromper, registre a ação exata usada.

## Aplicativos de tela inteira — minutos 19 a 27

Use cerca de dois minutos por aplicativo e dois minutos para comparar saídas comuns:

- `nvim`: abra um arquivo descartável, percorra linhas, selecione, cole as 200 linhas, redimensione e saia sem salvar. Observe cursor de inserção versus navegação, região de rolagem e restauração do shell.
- `lazygit`: abra o repositório descartável, navegue por listas e diff, use roda/clique quando habilitados, redimensione e saia. Não execute operações destrutivas. Cabeçalhos, cores de fundo e rodapés devem permanecer estáveis.
- `btop`: observe algumas atualizações, role/clique se suportado, altere o tamanho e saia. Não deve aparecer uma mistura de duas atualizações; o app deve recuperar o shell ao sair.
- No shell, confira `git log --color=always` e `git diff --color=always`; role e selecione texto. Teste também as sequências abaixo no PowerShell.

```powershell
$esc = [char]27
Write-Host "$esc[9mtachado$esc[29m normal $esc[2mesmaecido$esc[22m $esc[3mitálico$esc[23m $esc[4:3msublinhado$esc[24m $esc[53msobrelinha$esc[55m"
0..6 | ForEach-Object { Write-Host -NoNewline "$esc[$_ q"; Start-Sleep -Milliseconds 600 }
Write-Host -NoNewline "$esc[2 q"
Write-Host -NoNewline "$esc[?2026h"; 1..20 | ForEach-Object { Write-Host "linha $_" }; Write-Host -NoNewline "$esc[?2026l"
Write-Host -NoNewline "$esc[?2026hsem encerramento"; Start-Sleep -Milliseconds 300; Write-Host -NoNewline "$esc[?2026l"
```

As atualizações sincronizadas aparecem juntas ao terminar; um programa que esquecer de terminar não pode congelar a tela por mais que aproximadamente 150 ms. `4:3` é apresentado como sublinhado simples nesta versão. O cursor só pisca se o programa pedir uma forma piscante; deixe a Forja parada e confira que não voltou a redesenhar continuamente.

## Registrar — minutos 27 a 30

Para cada defeito, anote: ferramenta/versão, commit da Forja, tamanho/escala, shell, passos e teclas exatos, saída esperada e observada, frequência e se desaparece ao redimensionar. Guarde uma captura antes/depois e um trecho mínimo de sequência VT quando possível; remova prompts, caminhos privados e segredos antes de compartilhar. Marque cada ferramenta como aprovada, falhou ou não executada; não transforme “não executada” em aprovação.

Inclua se o problema ocorreu com tela alternativa, sincronização, foco, mouse, colagem, caracteres largos ou cursor. Um relato reproduzível deve virar primeiro um caso em `vt/parser_tests.rs`, `view/input.rs` ou nos testes de renderização, e só depois uma correção.
