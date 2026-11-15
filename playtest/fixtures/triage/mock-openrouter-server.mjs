// Offline mock OpenRouter SSE server used only by playtest/test-triage.ps1.
import fs from "node:fs";
import http from "node:http";

const [portFile, captureFile, mode = "ok"] = process.argv.slice(2);
if (!portFile || !captureFile) {
  throw new Error("Usage: node mock-openrouter-server.mjs <portFile> <captureFile> [mode]");
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", (chunk) => {
    body += chunk;
  });
  req.on("end", () => {
    let parsed = null;
    try {
      parsed = JSON.parse(body);
    } catch {
      parsed = { raw: body };
    }
    fs.appendFileSync(
      captureFile,
      JSON.stringify({ url: req.url, method: req.method, body: parsed }) + "\n",
      "utf8"
    );

    const model = String(parsed?.model ?? "");
    if (mode === "all429" || (mode === "fallback" && model === "z-ai/glm-5.3-flash")) {
      res.writeHead(429, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          error: { message: "No allowed providers are available or rate limit exceeded", code: 429 },
        })
      );
      return;
    }

    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    const chunk1 = {
      id: "gen-fixture-1",
      model,
      object: "chat.completion.chunk",
      created: 1700000000,
      choices: [
        {
          index: 0,
          delta: { role: "assistant", content: '{"priority":4,"area":"area-editor",' },
          finish_reason: null,
        },
      ],
    };
    const chunk2 = {
      id: "gen-fixture-1",
      model,
      object: "chat.completion.chunk",
      created: 1700000000,
      choices: [
        {
          index: 0,
          delta: {
            content: '"rationale":"Classificado via OpenRouter SDK streaming.","duplicate_of":null}',
          },
          finish_reason: "stop",
        },
      ],
      usage: {
        prompt_tokens: 120,
        completion_tokens: 45,
        total_tokens: 165,
        completion_tokens_details: { reasoning_tokens: 18 },
        cost: 0,
      },
    };
    res.write(`data: ${JSON.stringify(chunk1)}\n\n`);
    res.write(`data: ${JSON.stringify(chunk2)}\n\n`);
    res.write("data: [DONE]\n\n");
    res.end();
  });
});

server.listen(0, "127.0.0.1", () => {
  const addr = server.address();
  fs.writeFileSync(portFile, String(addr.port), "utf8");
});
