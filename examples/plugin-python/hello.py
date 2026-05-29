#!/usr/bin/env python3
"""Example metarepo plugin (Python). Speaks the v1 protocol over stdin/stdout.

Outside metarepo:
    ./hello.py               prints a banner and exits 0.
Under metarepo:
    METAREPO_PLUGIN_MODE=1 is set, and the host writes newline-delimited JSON
    requests on stdin and reads responses on stdout.

Install:
    chmod +x hello.py
    meta plugin install hello --from file:./hello.py
    meta hello greet Ada
"""

import json
import os
import sys

PROTOCOL_VERSION = "1.0"

PLUGIN = {
    "name": "hello",
    "version": "0.1.0",
    "experimental": False,
}


def commands():
    return [
        {
            "name": "hello",
            "about": "Greeting commands",
            "args": [],
            "subcommands": [
                {
                    "name": "greet",
                    "about": "Print a greeting",
                    "args": [{"name": "name", "help": "Name to greet", "required": True}],
                    "subcommands": [],
                }
            ],
        }
    ]


def handle(command, args, config):
    if args[:1] == ["greet"]:
        name = args[1] if len(args) > 1 else "world"
        return f"Hello, {name}! (cwd: {config.get('working_dir')})"
    return "usage: meta hello greet <name>"


def dispatch(req):
    t = req.get("type")
    if t == "GetInfo":
        return {
            "type": "Info",
            "name": PLUGIN["name"],
            "version": PLUGIN["version"],
            "experimental": PLUGIN["experimental"],
            "protocol_version": PROTOCOL_VERSION,
        }
    if t == "RegisterCommands":
        return {"type": "Commands", "commands": commands()}
    if t == "HandleCommand":
        try:
            message = handle(req["command"], req.get("args", []), req.get("config", {}))
            return {"type": "Success", "message": message}
        except Exception as e:
            return {"type": "Error", "message": str(e)}
    return {"type": "Error", "message": f"unknown request type: {t}"}


def serve():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            resp = dispatch(req)
        except json.JSONDecodeError as e:
            resp = {"type": "Error", "message": f"bad request: {e}"}
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()


def main():
    if os.environ.get("METAREPO_PLUGIN_MODE") == "1":
        serve()
    else:
        print(f"{PLUGIN['name']} v{PLUGIN['version']} — metarepo plugin (run via 'meta {PLUGIN['name']}')")


if __name__ == "__main__":
    main()
