// Example metarepo plugin (Go). Speaks the v1 protocol over stdin/stdout.
//
// Outside metarepo:
//   ./metarepo-plugin-hello       prints a banner and exits 0.
// Under metarepo:
//   METAREPO_PLUGIN_MODE=1 is set, and the host writes newline-delimited JSON
//   requests on stdin and reads responses on stdout.
//
// Build & install:
//   go build -o metarepo-plugin-hello
//   meta plugin install hello --from file:./metarepo-plugin-hello
//   meta hello greet Ada

package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
)

const protocolVersion = "1.0"

const (
	pluginName    = "hello"
	pluginVersion = "0.1.0"
)

type argInfo struct {
	Name     string `json:"name"`
	Help     string `json:"help"`
	Required bool   `json:"required"`
}

type commandInfo struct {
	Name        string        `json:"name"`
	About       string        `json:"about"`
	Args        []argInfo     `json:"args"`
	Subcommands []commandInfo `json:"subcommands"`
}

type request struct {
	Type    string          `json:"type"`
	Command string          `json:"command,omitempty"`
	Args    []string        `json:"args,omitempty"`
	Config  json.RawMessage `json:"config,omitempty"`
}

type response map[string]any

func commands() []commandInfo {
	return []commandInfo{{
		Name:  "hello",
		About: "Greeting commands",
		Args:  []argInfo{},
		Subcommands: []commandInfo{{
			Name:        "greet",
			About:       "Print a greeting",
			Args:        []argInfo{{Name: "name", Help: "Name to greet", Required: true}},
			Subcommands: []commandInfo{},
		}},
	}}
}

func handle(command string, args []string, config json.RawMessage) (string, error) {
	if len(args) > 0 && args[0] == "greet" {
		name := "world"
		if len(args) > 1 {
			name = args[1]
		}
		var cfg struct {
			WorkingDir string `json:"working_dir"`
		}
		_ = json.Unmarshal(config, &cfg)
		return fmt.Sprintf("Hello, %s! (cwd: %s)", name, cfg.WorkingDir), nil
	}
	return "usage: meta hello greet <name>", nil
}

func dispatch(req request) response {
	switch req.Type {
	case "GetInfo":
		return response{
			"type":             "Info",
			"name":             pluginName,
			"version":          pluginVersion,
			"experimental":     false,
			"protocol_version": protocolVersion,
		}
	case "RegisterCommands":
		return response{"type": "Commands", "commands": commands()}
	case "HandleCommand":
		msg, err := handle(req.Command, req.Args, req.Config)
		if err != nil {
			return response{"type": "Error", "message": err.Error()}
		}
		return response{"type": "Success", "message": msg}
	default:
		return response{"type": "Error", "message": "unknown request type: " + req.Type}
	}
}

func serve() {
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	out := bufio.NewWriter(os.Stdout)
	enc := json.NewEncoder(out)
	for in.Scan() {
		line := in.Bytes()
		if len(line) == 0 {
			continue
		}
		var req request
		var resp response
		if err := json.Unmarshal(line, &req); err != nil {
			resp = response{"type": "Error", "message": "bad request: " + err.Error()}
		} else {
			resp = dispatch(req)
		}
		_ = enc.Encode(resp)
		_ = out.Flush()
	}
}

func main() {
	if os.Getenv("METAREPO_PLUGIN_MODE") == "1" {
		serve()
		return
	}
	fmt.Printf("%s v%s — metarepo plugin (run via 'meta %s')\n", pluginName, pluginVersion, pluginName)
}
