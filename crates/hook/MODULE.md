# forja-hook

`forja-hook.exe` lê **um** payload do stdin e repassa ao app pelo pipe, como o
método `hook.event`. A interpretação por agente (Claude Code, Codex, …) chega
na Fase C.

## Contrato

- payload JSON vira `params.payload`;
- payload não-JSON vira uma string;
- stdin vazio termina com código 0;
- app fechado termina com código 0 sem esperar;
- app aberto sem o método `hook.event` ainda termina com 0 (transporte
  funcionou, handler chega na Fase C);
- erro de transporte com o app aberto termina com código 1.

## Como testar

```sh
echo '{"event":"done"}' | ./target/debug/forja-hook.exe
```

Com o app fechado, a execução termina em milissegundos.
