#!/usr/bin/env node
// Example metarepo plugin (Node.js). Speaks the v1 protocol over stdin/stdout.
//
// Usage outside metarepo:
//   ./hello.mjs           prints a banner and exits 0.
// Under metarepo:
//   METAREPO_PLUGIN_MODE=1 is set, and the host writes newline-delimited JSON
//   requests on stdin and reads responses on stdout.
//
// Install:
//   chmod +x hello.mjs
//   meta plugin install hello --from file:./hello.mjs
//   meta hello greet Ada

import { createInterface } from "node:readline";

const PROTOCOL_VERSION = "1.0";

const plugin = {
  name: "hello",
  version: "0.1.0",
  experimental: false,

  commands() {
    return [
      {
        name: "hello",
        about: "Greeting commands",
        args: [],
        subcommands: [
          {
            name: "greet",
            about: "Print a greeting",
            args: [{ name: "name", help: "Name to greet", required: true }],
            subcommands: [],
          },
        ],
      },
    ];
  },

  handle(command, args, config) {
    const [sub, ...rest] = args;
    if (sub === "greet") {
      const name = rest[0] ?? "world";
      return { message: `Hello, ${name}! (cwd: ${config.working_dir})` };
    }
    return { message: "usage: meta hello greet <name>" };
  },
};

function dispatch(req) {
  switch (req.type) {
    case "GetInfo":
      return {
        type: "Info",
        name: plugin.name,
        version: plugin.version,
        experimental: plugin.experimental,
        protocol_version: PROTOCOL_VERSION,
      };
    case "RegisterCommands":
      return { type: "Commands", commands: plugin.commands() };
    case "HandleCommand":
      try {
        const { message = null } = plugin.handle(req.command, req.args, req.config) ?? {};
        return { type: "Success", message };
      } catch (e) {
        return { type: "Error", message: String(e?.message ?? e) };
      }
    default:
      return { type: "Error", message: `unknown request type: ${req.type}` };
  }
}

function serve() {
  const rl = createInterface({ input: process.stdin });
  rl.on("line", (line) => {
    if (!line.trim()) return;
    let resp;
    try {
      resp = dispatch(JSON.parse(line));
    } catch (e) {
      resp = { type: "Error", message: `bad request: ${String(e?.message ?? e)}` };
    }
    process.stdout.write(JSON.stringify(resp) + "\n");
  });
  rl.on("close", () => process.exit(0));
}

if (process.env.METAREPO_PLUGIN_MODE === "1") {
  serve();
} else {
  console.log(`${plugin.name} v${plugin.version} — metarepo plugin (run via 'meta ${plugin.name}')`);
}
