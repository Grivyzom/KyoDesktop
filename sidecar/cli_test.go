package main

import (
	"encoding/json"
	"testing"
)

func TestStripRemoteCredentials(t *testing.T) {
	cases := []struct {
		in   string
		want string
	}{
		{"https://user:ghp_secret@github.com/org/repo.git", "https://github.com/org/repo.git"},
		{"git@github.com:org/repo.git", "git@github.com:org/repo.git"},
		{"ssh://deploy:tok@git.example.com/repo.git", "ssh://git.example.com/repo.git"},
	}
	for _, tc := range cases {
		if got := stripRemoteCredentials(tc.in); got != tc.want {
			t.Errorf("stripRemoteCredentials(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

func TestParseOriginFromGitConfig(t *testing.T) {
	cfg := `[core]
	repositoryformatversion = 0
[remote "upstream"]
	url = https://github.com/other/repo.git
[remote "origin"]
	url = git@github.com:Grivyzom/KyoDesktop.git
	fetch = +refs/heads/*:refs/remotes/origin/*
`
	if got := parseOriginFromGitConfig(cfg); got != "git@github.com:Grivyzom/KyoDesktop.git" {
		t.Fatalf("got %q", got)
	}
}

func TestExtractRPCJSONFromSSE(t *testing.T) {
	sse := "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n"
	got := string(extractRPCJSONFromHTTP("text/event-stream", []byte(sse)))
	want := `{"jsonrpc":"2.0","id":1,"result":{"ok":true}}`
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestMergeLocalProjectTools(t *testing.T) {
	in := json.RawMessage(`{"tools":[{"name":"get_space"},{"name":"project_status"}]}`)
	out := mergeLocalProjectTools(in)
	var parsed struct {
		Tools []struct {
			Name string `json:"name"`
		} `json:"tools"`
	}
	if err := json.Unmarshal(out, &parsed); err != nil {
		t.Fatal(err)
	}
	if len(parsed.Tools) != 5 {
		t.Fatalf("expected 5 tools (1 remote + 4 local), got %d", len(parsed.Tools))
	}
}

func TestFindPendingEditID(t *testing.T) {
	root := "/tmp/proj"
	transcript := `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"ed1","name":"Edit","input":{"file_path":"/tmp/proj/src/main.rs"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"ed2","name":"Edit","input":{"file_path":"/tmp/proj/CLAUDE.md"}}]}}`
	if got := findPendingEditID(transcript, root); got != "ed1" {
		t.Fatalf("expected ed1, got %q", got)
	}
	transcript2 := transcript + "\n" + `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"sp1","name":"mcp__kyo__update_space_page","input":{}}]}}`
	if got := findPendingEditID(transcript2, root); got != "" {
		t.Fatalf("expected empty after space update, got %q", got)
	}
}
