package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"sync"
	"time"
)

const (
	defaultKyoServerURL = "https://kyo.broco.dev"
	kyoSidecarVersion   = "0.1.0"
	kyoSidecarUserAgent = "kyo-desktop/0.1.0"
	dpapiPrefix         = "dpapi:v1:"
	projectFileName     = "kyo-project.json"
)

// runKyoCLISubcommand despacha los subcomandos nativos equivalentes a kyo-cli:
// mcp, statusline y hook stop.
func runKyoCLISubcommand(args []string) int {
	if len(args) == 0 {
		return 1
	}
	switch args[0] {
	case "mcp":
		return runMCPBridge(args[1:])
	case "statusline":
		return runStatusLine(args[1:])
	case "hook":
		if len(args) > 1 && args[1] == "stop" {
			return runHookStop()
		}
		return 0
	default:
		return 1
	}
}

// ── Configuración y Sesión compartida (KyoDesktop + kyo-cli) ──────────────

func kyoConfigDir() string {
	if runtime.GOOS == "windows" {
		if appData := os.Getenv("APPDATA"); appData != "" {
			return filepath.Join(appData, "kyo")
		}
	}
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "kyo")
	}
	home, _ := os.UserHomeDir()
	if runtime.GOOS == "darwin" {
		return filepath.Join(home, "Library", "Application Support", "kyo")
	}
	return filepath.Join(home, ".config", "kyo")
}

func kyoCLIConfigDir() string {
	if d := os.Getenv("KYO_CONFIG_DIR"); d != "" {
		return d
	}
	if runtime.GOOS == "windows" {
		if appData := os.Getenv("APPDATA"); appData != "" {
			return filepath.Join(appData, "kyo-cli")
		}
	}
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "kyo-cli")
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", "kyo-cli")
}

func decryptDiskToken(raw string) string {
	trimmed := strings.TrimSpace(raw)
	if trimmed == "" {
		return ""
	}
	if strings.HasPrefix(trimmed, dpapiPrefix) {
		b64 := strings.TrimPrefix(trimmed, dpapiPrefix)
		cipher, err := base64.StdEncoding.DecodeString(b64)
		if err != nil {
			cipher, err = base64.RawStdEncoding.DecodeString(b64)
		}
		if err != nil {
			return ""
		}
		plain, ok := dpapiDecryptBytes(cipher)
		if !ok {
			return ""
		}
		return strings.TrimSpace(string(plain))
	}
	return trimmed
}

func normalizeKyoServerURL(raw string) (string, error) {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || u.Scheme == "" || u.Host == "" {
		return "", fmt.Errorf("URL de servidor inválida: %s", raw)
	}
	host := u.Hostname()
	isLoopback := host == "localhost" || host == "127.0.0.1" || host == "::1"
	if u.Scheme != "https" && !(u.Scheme == "http" && isLoopback) {
		return "", errors.New("el servidor debe usar https:// (http:// sólo para localhost)")
	}
	p := strings.TrimRight(u.Path, "/")
	p = strings.TrimSuffix(p, "/api/v1")
	return fmt.Sprintf("%s://%s%s", u.Scheme, u.Host, p), nil
}

type desktopSession struct {
	ServerURL    string
	Token        string
	RefreshToken string
	ClientID     string
	ExpiresAt    string
}

func loadAccountSession(flagURL string) (desktopSession, error) {
	// 1. Precedencia de token por entorno
	envTok := strings.TrimSpace(os.Getenv("KYO_TOKEN"))
	if envTok == "" {
		envTok = strings.TrimSpace(os.Getenv("KYO_AUTH_TOKEN"))
	}

	// 2. Leer session.json de Kyo Desktop
	var sess desktopSession
	sessionPath := filepath.Join(kyoConfigDir(), "session.json")
	if raw, err := os.ReadFile(sessionPath); err == nil {
		var m map[string]any
		if json.Unmarshal(raw, &m) == nil {
			if t, ok := m["token"].(string); ok {
				sess.Token = decryptDiskToken(t)
			}
			if rt, ok := m["refresh_token"].(string); ok {
				sess.RefreshToken = decryptDiskToken(rt)
			}
			if u, ok := m["server_url"].(string); ok && strings.TrimSpace(u) != "" {
				sess.ServerURL = strings.TrimSpace(u)
			}
			if cid, ok := m["oauth_client_id"].(string); ok && strings.TrimSpace(cid) != "" {
				sess.ClientID = strings.TrimSpace(cid)
			}
			if exp, ok := m["access_expires_at"].(string); ok {
				sess.ExpiresAt = strings.TrimSpace(exp)
			}
		}
	}

	// 3. Fallback a kyo-cli/config.json si session.json no tiene token
	if sess.Token == "" {
		cliPath := filepath.Join(kyoCLIConfigDir(), "config.json")
		if raw, err := os.ReadFile(cliPath); err == nil {
			var cfg struct {
				ServerURL    string `json:"serverUrl"`
				AccessToken  string `json:"accessToken"`
				RefreshToken string `json:"refreshToken"`
				ExpiresAt    string `json:"accessExpiresAt"`
			}
			if json.Unmarshal(raw, &cfg) == nil && cfg.AccessToken != "" {
				sess.Token = cfg.AccessToken
				sess.RefreshToken = cfg.RefreshToken
				sess.ExpiresAt = cfg.ExpiresAt
				if sess.ServerURL == "" {
					sess.ServerURL = cfg.ServerURL
				}
				if sess.ClientID == "" {
					sess.ClientID = "kyo-cli"
				}
			}
		}
	}

	if envTok != "" {
		sess.Token = envTok
		sess.RefreshToken = ""
	}

	rawURL := flagURL
	if rawURL == "" {
		rawURL = strings.TrimSpace(os.Getenv("KYO_SERVER_URL"))
	}
	if rawURL == "" {
		rawURL = strings.TrimSpace(os.Getenv("KYO_UPSTREAM"))
	}
	if rawURL == "" {
		rawURL = sess.ServerURL
	}
	if rawURL == "" {
		rawURL = defaultKyoServerURL
	}
	normURL, err := normalizeKyoServerURL(rawURL)
	if err != nil {
		return desktopSession{}, err
	}
	if envTok == "" && sess.ServerURL != "" {
		if storedNorm, err := normalizeKyoServerURL(sess.ServerURL); err == nil && storedNorm != normURL {
			return desktopSession{}, errors.New("la sesión guardada pertenece a otro servidor; inicia sesión en Kyo Desktop")
		}
	}
	sess.ServerURL = normURL
	if sess.Token == "" {
		return desktopSession{}, errors.New("no hay sesión iniciada en Kyo Desktop ni kyo-cli")
	}
	if sess.ClientID == "" {
		sess.ClientID = "kyo-desktop"
	}
	return sess, nil
}

// ── Git, Huella y Archivos de Proyecto ─────────────────────────────────────

type gitRemoteInfo struct {
	Name string `json:"name"`
	URL  string `json:"url"`
}

type projectLocation struct {
	Hostname string          `json:"hostname"`
	Path     string          `json:"path"`
	Branch   string          `json:"branch"`
	Remotes  []gitRemoteInfo `json:"remotes"`
}

type projectRef struct {
	GitRemote string `json:"git_remote,omitempty"`
	PathHash  string `json:"path_hash,omitempty"`
}

type projectFile struct {
	SpaceID         string `json:"space_id"`
	ProjectID       string `json:"project_id,omitempty"`
	TemplateSpaceID string `json:"template_space_id"`
	TemplateVersion string `json:"template_version"`
	LinkedAt        string `json:"linked_at"`
	ServerURL       string `json:"server_url,omitempty"`
	AutoLog         bool   `json:"auto_log,omitempty"`
}

func isSymlinkPath(p string) bool {
	info, err := os.Lstat(p)
	if err != nil {
		return false
	}
	return info.Mode()&os.ModeSymlink != 0
}

func safeWriteAtomic(file string, data []byte, perm os.FileMode) error {
	if isSymlinkPath(file) {
		return fmt.Errorf("%s es un enlace simbólico: por seguridad no se escribe a través de él", file)
	}
	var rnd [6]byte
	_, _ = rand.Read(rnd[:])
	tmp := filepath.Join(filepath.Dir(file), fmt.Sprintf(".%s.%s.tmp", filepath.Base(file), hex.EncodeToString(rnd[:])))
	f, err := os.OpenFile(tmp, os.O_CREATE|os.O_EXCL|os.O_WRONLY, perm)
	if err != nil {
		return err
	}
	if _, err := f.Write(data); err != nil {
		_ = f.Close()
		_ = os.Remove(tmp)
		return err
	}
	if err := f.Close(); err != nil {
		_ = os.Remove(tmp)
		return err
	}
	if err := os.Rename(tmp, file); err != nil {
		_ = os.Remove(tmp)
		return err
	}
	return nil
}

func findProjectRootDir(cwd string) string {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	cmd.Dir = cwd
	if out, err := cmd.Output(); err == nil {
		if top := strings.TrimSpace(string(out)); top != "" {
			if abs, err := filepath.Abs(top); err == nil {
				return abs
			}
		}
	}
	cur, err := filepath.Abs(cwd)
	if err != nil {
		return cwd
	}
	for {
		if _, err := os.Stat(filepath.Join(cur, ".git")); err == nil {
			return cur
		}
		parent := filepath.Dir(cur)
		if parent == cur {
			abs, _ := filepath.Abs(cwd)
			return abs
		}
		cur = parent
	}
}

func stripRemoteCredentials(remote string) string {
	if !regexp.MustCompile(`(?i)^[a-z][a-z0-9+.-]*://`).MatchString(remote) {
		return remote
	}
	u, err := url.Parse(remote)
	if err != nil {
		return regexp.MustCompile(`//[^/@]*@`).ReplaceAllString(remote, "//")
	}
	u.User = nil
	return u.String()
}

func parseOriginFromGitConfig(cfg string) string {
	inOrigin := false
	for _, raw := range strings.Split(cfg, "\n") {
		line := strings.TrimSpace(raw)
		if strings.HasPrefix(line, `[remote "origin"]`) {
			inOrigin = true
		} else if strings.HasPrefix(line, "[") {
			inOrigin = false
		} else if inOrigin && strings.HasPrefix(line, "url") {
			if parts := strings.SplitN(line, "=", 2); len(parts) == 2 {
				return strings.TrimSpace(parts[1])
			}
		}
	}
	return ""
}

func readOriginRemoteDir(root string) string {
	gitPath := filepath.Join(root, ".git")
	info, err := os.Stat(gitPath)
	if err == nil && info.IsDir() {
		raw, err := os.ReadFile(filepath.Join(gitPath, "config"))
		if err != nil {
			return ""
		}
		return parseOriginFromGitConfig(string(raw))
	}
	cmd := exec.Command("git", "config", "--get", "remote.origin.url")
	cmd.Dir = root
	out, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func computeProjectRef(root string) projectRef {
	if r := readOriginRemoteDir(root); r != "" {
		return projectRef{GitRemote: stripRemoteCredentials(r)}
	}
	host, _ := os.Hostname()
	abs, _ := filepath.Abs(root)
	sum := sha256.Sum256([]byte(host + "|" + abs))
	return projectRef{PathHash: hex.EncodeToString(sum[:])}
}

func readProjectFileLocal(root string) *projectFile {
	p := filepath.Join(root, projectFileName)
	if isSymlinkPath(p) {
		return nil
	}
	raw, err := os.ReadFile(p)
	if err != nil {
		return nil
	}
	var pf projectFile
	if json.Unmarshal(raw, &pf) != nil || pf.SpaceID == "" {
		return nil
	}
	return &pf
}

func readProjectTokenLocal(root string) string {
	p := filepath.Join(root, ".kyo", "credentials")
	if isSymlinkPath(p) {
		return ""
	}
	raw, err := os.ReadFile(p)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(raw))
}

func collectLocationInfo(root string) projectLocation {
	host, _ := os.Hostname()
	abs, _ := filepath.Abs(root)
	branch := ""
	if head, err := os.ReadFile(filepath.Join(root, ".git", "HEAD")); err == nil {
		h := strings.TrimSpace(string(head))
		if strings.HasPrefix(h, "ref: refs/heads/") {
			branch = strings.TrimPrefix(h, "ref: refs/heads/")
		}
	}
	var remotes []gitRemoteInfo
	if cfg, err := os.ReadFile(filepath.Join(root, ".git", "config")); err == nil {
		current := ""
		reHdr := regexp.MustCompile(`^\[remote "([^"]+)"\]$`)
		reURL := regexp.MustCompile(`^url\s*=`)
		for _, raw := range strings.Split(string(cfg), "\n") {
			line := strings.TrimSpace(raw)
			if m := reHdr.FindStringSubmatch(line); len(m) == 2 {
				current = m[1]
			} else if strings.HasPrefix(line, "[") {
				current = ""
			} else if current != "" && reURL.MatchString(line) {
				if idx := strings.Index(line, "="); idx != -1 {
					u := strings.TrimSpace(line[idx+1:])
					if u != "" {
						remotes = append(remotes, gitRemoteInfo{Name: current, URL: stripRemoteCredentials(u)})
					}
				}
			}
		}
	}
	if remotes == nil {
		remotes = []gitRemoteInfo{}
	}
	return projectLocation{Hostname: host, Path: abs, Branch: branch, Remotes: remotes}
}

// ── Puente MCP stdio -> Streamable HTTP (/api/v1/mcp) ──────────────────────

type rpcRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

type rpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Result  any             `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

type rpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func runMCPBridge(args []string) int {
	var flagURL string
	var projectMode bool
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "-u", "--url":
			if i+1 < len(args) {
				flagURL = args[i+1]
				i++
			}
		case "--project":
			projectMode = true
		}
	}

	cwd, _ := os.Getwd()
	root := findProjectRootDir(cwd)

	var serverURL, bearer string
	var sess *desktopSession

	if projectMode {
		tok := readProjectTokenLocal(root)
		if tok == "" {
			return serveMCPSetupError(fmt.Sprintf("No hay .kyo/credentials en %s. Vincula el proyecto desde Kyo Desktop o `kyo-cli project link`.", root))
		}
		trusted := defaultKyoServerURL
		if flagURL != "" {
			u, err := normalizeKyoServerURL(flagURL)
			if err != nil {
				return serveMCPSetupError(err.Error())
			}
			trusted = u
		} else if s, err := loadAccountSession(""); err == nil {
			trusted = s.ServerURL
		}
		if pf := readProjectFileLocal(root); pf != nil && pf.ServerURL != "" {
			declared, err := normalizeKyoServerURL(pf.ServerURL)
			if err != nil {
				return serveMCPSetupError(fmt.Sprintf("kyo-project.json declara un server_url inválido (%s).", pf.ServerURL))
			}
			if declared != trusted {
				return serveMCPSetupError(fmt.Sprintf("kyo-project.json apunta a %s, pero tu servidor confiable es %s.", declared, trusted))
			}
		}
		serverURL = trusted
		bearer = tok
	} else {
		s, err := loadAccountSession(flagURL)
		if err != nil {
			return serveMCPSetupError(err.Error())
		}
		sess = &s
		serverURL = s.ServerURL
		bearer = s.Token
	}

	endpoint := serverURL + "/api/v1/mcp"
	httpClient := &http.Client{
		Timeout: 60 * time.Second,
		CheckRedirect: func(*http.Request, []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}

	var mu sync.Mutex
	var mcpSessionID string

	callRemote := func(method string, id json.RawMessage, params json.RawMessage) (json.RawMessage, *rpcError) {
		payload, _ := json.Marshal(map[string]any{
			"jsonrpc": "2.0",
			"id":      json.RawMessage(id),
			"method":  method,
			"params":  json.RawMessage(nonEmptyJSON(params)),
		})
		doPost := func(tok string) (*http.Response, error) {
			req, err := http.NewRequest(http.MethodPost, endpoint, bytes.NewReader(payload))
			if err != nil {
				return nil, err
			}
			req.Header.Set("Authorization", "Bearer "+tok)
			req.Header.Set("Content-Type", "application/json")
			req.Header.Set("Accept", "application/json, text/event-stream")
			req.Header.Set("User-Agent", kyoSidecarUserAgent)
			mu.Lock()
			sid := mcpSessionID
			mu.Unlock()
			if sid != "" {
				req.Header.Set("Mcp-Session-Id", sid)
			}
			return httpClient.Do(req)
		}

		res, err := doPost(bearer)
		if err != nil {
			return nil, &rpcError{Code: -32000, Message: fmt.Sprintf("No se pudo conectar con %s: %v", endpoint, err)}
		}
		if res.StatusCode == http.StatusUnauthorized && sess != nil && sess.RefreshToken != "" {
			_ = res.Body.Close()
			if refreshed, rerr := refreshOAuthTokenGo(httpClient, *sess); rerr == nil {
				bearer = refreshed
				res, err = doPost(bearer)
				if err != nil {
					return nil, &rpcError{Code: -32000, Message: err.Error()}
				}
			}
		}
		defer res.Body.Close()

		if sid := res.Header.Get("Mcp-Session-Id"); sid != "" {
			mu.Lock()
			mcpSessionID = sid
			mu.Unlock()
		}

		bodyBytes, _ := io.ReadAll(res.Body)
		if res.StatusCode < 200 || res.StatusCode >= 300 {
			return nil, &rpcError{Code: -32000, Message: fmt.Sprintf("Error HTTP %d desde Kyō MCP: %s", res.StatusCode, strings.TrimSpace(string(bodyBytes)))}
		}

		jsonBody := extractRPCJSONFromHTTP(res.Header.Get("Content-Type"), bodyBytes)
		var env struct {
			Result json.RawMessage `json:"result"`
			Error  *rpcError       `json:"error"`
		}
		if err := json.Unmarshal(jsonBody, &env); err != nil {
			return nil, &rpcError{Code: -32603, Message: fmt.Sprintf("Respuesta MCP inválida: %v", err)}
		}
		return env.Result, env.Error
	}

	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	enc := json.NewEncoder(os.Stdout)

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		var req rpcRequest
		if err := json.Unmarshal([]byte(line), &req); err != nil {
			continue
		}
		// Notificaciones (sin id)
		if len(req.ID) == 0 || string(req.ID) == "null" {
			continue
		}

		switch req.Method {
		case "initialize":
			res, rerr := callRemote("initialize", req.ID, req.Params)
			if rerr != nil {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Error: rerr})
			} else {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: res})
			}
		case "tools/list":
			res, rerr := callRemote("tools/list", req.ID, req.Params)
			if rerr != nil {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Error: rerr})
				continue
			}
			if !projectMode {
				res = mergeLocalProjectTools(res)
			}
			_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: res})
		case "tools/call":
			var p struct {
				Name      string         `json:"name"`
				Arguments map[string]any `json:"arguments"`
			}
			_ = json.Unmarshal(req.Params, &p)
			if !projectMode && isLocalProjectToolName(p.Name) {
				out := runLocalProjectToolGo(httpClient, serverURL, bearer, root, p.Name, p.Arguments)
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: out})
				continue
			}
			res, rerr := callRemote("tools/call", req.ID, req.Params)
			if rerr != nil {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Error: rerr})
			} else {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: res})
			}
		default:
			res, rerr := callRemote(req.Method, req.ID, req.Params)
			if rerr != nil {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Error: rerr})
			} else {
				_ = enc.Encode(rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: res})
			}
		}
	}
	return 0
}

func nonEmptyJSON(raw json.RawMessage) json.RawMessage {
	if len(bytes.TrimSpace(raw)) == 0 {
		return json.RawMessage(`{}`)
	}
	return raw
}

func extractRPCJSONFromHTTP(contentType string, body []byte) []byte {
	if strings.Contains(strings.ToLower(contentType), "text/event-stream") || bytes.HasPrefix(bytes.TrimSpace(body), []byte("event:")) || bytes.HasPrefix(bytes.TrimSpace(body), []byte("data:")) {
		var lastData []byte
		for _, line := range strings.Split(string(body), "\n") {
			line = strings.TrimSpace(line)
			if strings.HasPrefix(line, "data:") {
				d := strings.TrimSpace(strings.TrimPrefix(line, "data:"))
				if d != "" && d != "[DONE]" {
					lastData = []byte(d)
				}
			}
		}
		if len(lastData) > 0 {
			return lastData
		}
	}
	return body
}

func refreshOAuthTokenGo(client *http.Client, sess desktopSession) (string, error) {
	form := url.Values{}
	form.Set("grant_type", "refresh_token")
	form.Set("client_id", sess.ClientID)
	form.Set("refresh_token", sess.RefreshToken)
	req, err := http.NewRequest(http.MethodPost, sess.ServerURL+"/oauth/token", strings.NewReader(form.Encode()))
	if err != nil {
		return "", err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", kyoSidecarUserAgent)
	res, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return "", fmt.Errorf("refresh HTTP %d", res.StatusCode)
	}
	var tok struct {
		AccessToken string `json:"access_token"`
	}
	if err := json.NewDecoder(res.Body).Decode(&tok); err != nil || tok.AccessToken == "" {
		return "", errors.New("refresh sin access_token")
	}
	return tok.AccessToken, nil
}

func serveMCPSetupError(reason string) int {
	fmt.Fprintf(os.Stderr, "[kyo-desktop mcp] %s\n", reason)
	scanner := bufio.NewScanner(os.Stdin)
	enc := json.NewEncoder(os.Stdout)
	for scanner.Scan() {
		var req rpcRequest
		if json.Unmarshal(scanner.Bytes(), &req) != nil || len(req.ID) == 0 || string(req.ID) == "null" {
			continue
		}
		switch req.Method {
		case "initialize":
			_ = enc.Encode(rpcResponse{
				JSONRPC: "2.0",
				ID:      req.ID,
				Result: map[string]any{
					"protocolVersion": "2024-11-05",
					"capabilities":    map[string]any{"tools": map[string]any{}},
					"serverInfo":      map[string]any{"name": "kyo", "version": kyoSidecarVersion},
					"instructions":    "Kyō no se pudo iniciar: " + reason,
				},
			})
		case "tools/list":
			_ = enc.Encode(rpcResponse{
				JSONRPC: "2.0",
				ID:      req.ID,
				Result: map[string]any{
					"tools": []map[string]any{{
						"name":        "kyo_setup_status",
						"description": "Kyō no está disponible en este proyecto: devuelve el motivo y cómo arreglarlo.",
						"inputSchema": map[string]any{"type": "object", "properties": map[string]any{}},
					}},
				},
			})
		case "tools/call":
			_ = enc.Encode(rpcResponse{
				JSONRPC: "2.0",
				ID:      req.ID,
				Result: map[string]any{
					"content": []map[string]any{{"type": "text", "text": "Kyō no se pudo iniciar: " + reason}},
					"isError": true,
				},
			})
		default:
			_ = enc.Encode(rpcResponse{
				JSONRPC: "2.0",
				ID:      req.ID,
				Error:   &rpcError{Code: -32601, Message: "Método no disponible en modo setup"},
			})
		}
	}
	return 0
}

func isLocalProjectToolName(name string) bool {
	return name == "project_status" || name == "project_link" || name == "project_renew" || name == "project_unlink"
}

func mergeLocalProjectTools(raw json.RawMessage) json.RawMessage {
	var parsed map[string]any
	if json.Unmarshal(raw, &parsed) != nil {
		return raw
	}
	var filtered []any
	if existing, ok := parsed["tools"].([]any); ok {
		for _, item := range existing {
			if m, ok := item.(map[string]any); ok {
				if name, _ := m["name"].(string); strings.HasPrefix(name, "project_") {
					continue
				}
			}
			filtered = append(filtered, item)
		}
	}
	localDefs := []map[string]any{
		{
			"name":        "project_status",
			"description": "Estado del vínculo entre el proyecto actual y su Proyecto de Kyō.",
			"inputSchema": map[string]any{"type": "object", "properties": map[string]any{}},
		},
		{
			"name":        "project_link",
			"description": "Vincula el proyecto actual a Kyō (crea Espacio, Bóveda y Lienzo o adopta un proyecto existente).",
			"inputSchema": map[string]any{
				"type": "object",
				"properties": map[string]any{
					"project_id": map[string]any{"type": "string"},
					"no_tools":   map[string]any{"type": "boolean"},
					"name":       map[string]any{"type": "string"},
					"force":      map[string]any{"type": "boolean"},
				},
			},
		},
		{
			"name":        "project_renew",
			"description": "Genera un token de proyecto nuevo (30 días) para el mismo Proyecto.",
			"inputSchema": map[string]any{"type": "object", "properties": map[string]any{}},
		},
		{
			"name":        "project_unlink",
			"description": "Desvincula el proyecto actual conservando el Proyecto y sus herramientas en Kyō.",
			"inputSchema": map[string]any{"type": "object", "properties": map[string]any{}},
		},
	}
	for _, d := range localDefs {
		filtered = append(filtered, d)
	}
	parsed["tools"] = filtered
	out, err := json.Marshal(parsed)
	if err != nil {
		return raw
	}
	return out
}

func runLocalProjectToolGo(client *http.Client, serverURL, bearer, root, name string, args map[string]any) map[string]any {
	mcpText := func(s string, isErr bool) map[string]any {
		return map[string]any{
			"content": []map[string]any{{"type": "text", "text": s}},
			"isError": isErr,
		}
	}
	ref := computeProjectRef(root)
	apiCall := func(method, path string, body any) ([]byte, int, error) {
		var rdr io.Reader
		if body != nil {
			b, _ := json.Marshal(body)
			rdr = bytes.NewReader(b)
		}
		req, err := http.NewRequest(method, serverURL+"/api/v1"+path, rdr)
		if err != nil {
			return nil, 0, err
		}
		req.Header.Set("Authorization", "Bearer "+bearer)
		req.Header.Set("Accept", "application/json")
		req.Header.Set("User-Agent", kyoSidecarUserAgent)
		if body != nil {
			req.Header.Set("Content-Type", "application/json")
		}
		res, err := client.Do(req)
		if err != nil {
			return nil, 0, err
		}
		defer res.Body.Close()
		out, _ := io.ReadAll(res.Body)
		return out, res.StatusCode, nil
	}

	switch name {
	case "project_status":
		q := url.Values{}
		if ref.GitRemote != "" {
			q.Set("git_remote", ref.GitRemote)
		}
		if ref.PathHash != "" {
			q.Set("path_hash", ref.PathHash)
		}
		body, code, err := apiCall(http.MethodGet, "/project-links/status?"+q.Encode(), nil)
		if err != nil || code >= 400 {
			return mcpText(fmt.Sprintf("Error consultando estado (%d): %v %s", code, err, string(body)), true)
		}
		var remote any
		_ = json.Unmarshal(body, &remote)
		pretty, _ := json.MarshalIndent(map[string]any{
			"root":          root,
			"remote":        remote,
			"localFile":     readProjectFileLocal(root),
			"hasLocalToken": readProjectTokenLocal(root) != "",
		}, "", "  ")
		return mcpText(string(pretty), false)
	case "project_link":
		reqBody := map[string]any{
			"force":    args["force"] == true,
			"location": collectLocationInfo(root),
		}
		if ref.GitRemote != "" {
			reqBody["git_remote"] = ref.GitRemote
		} else {
			reqBody["path_hash"] = ref.PathHash
		}
		if pid, ok := args["project_id"].(string); ok && pid != "" {
			reqBody["project_id"] = pid
		} else {
			nameVal, _ := args["name"].(string)
			if strings.TrimSpace(nameVal) == "" {
				nameVal = filepath.Base(root)
			}
			noTools := args["no_tools"] == true
			reqBody["name"] = nameVal
			reqBody["no_tools"] = noTools
			reqBody["blank"] = !noTools
		}
		body, code, err := apiCall(http.MethodPost, "/project-links", reqBody)
		if err != nil || code >= 400 {
			return mcpText(fmt.Sprintf("Error vinculando proyecto (%d): %v %s", code, err, string(body)), true)
		}
		var res struct {
			SpaceID         string `json:"space_id"`
			ProjectID       string `json:"project_id"`
			TemplateSpaceID string `json:"template_space_id"`
			TemplateVersion string `json:"template_version"`
			LinkedAt        string `json:"linked_at"`
			ExpiresAt       string `json:"expires_at"`
			Token           string `json:"token"`
		}
		if err := json.Unmarshal(body, &res); err == nil && res.Token != "" {
			pf := projectFile{
				SpaceID:         res.SpaceID,
				ProjectID:       res.ProjectID,
				TemplateSpaceID: res.TemplateSpaceID,
				TemplateVersion: res.TemplateVersion,
				LinkedAt:        res.LinkedAt,
				ServerURL:       serverURL,
			}
			if rawPF, err := json.MarshalIndent(pf, "", "  "); err == nil {
				_ = safeWriteAtomic(filepath.Join(root, projectFileName), append(rawPF, '\n'), 0o644)
			}
			_ = os.MkdirAll(filepath.Join(root, ".kyo"), 0o700)
			_ = safeWriteAtomic(filepath.Join(root, ".kyo", "credentials"), []byte(res.Token+"\n"), 0o600)
		}
		return mcpText(fmt.Sprintf("Proyecto vinculado (space_id %s, vence %s).", res.SpaceID, res.ExpiresAt), false)
	case "project_renew":
		reqBody := map[string]any{"location": collectLocationInfo(root)}
		if ref.GitRemote != "" {
			reqBody["git_remote"] = ref.GitRemote
		} else {
			reqBody["path_hash"] = ref.PathHash
		}
		body, code, err := apiCall(http.MethodPost, "/project-links/renew", reqBody)
		if err != nil || code >= 400 {
			return mcpText(fmt.Sprintf("Error renovando token (%d): %v %s", code, err, string(body)), true)
		}
		var res struct {
			SpaceID   string `json:"space_id"`
			ExpiresAt string `json:"expires_at"`
			Token     string `json:"token"`
		}
		if err := json.Unmarshal(body, &res); err == nil && res.Token != "" {
			_ = os.MkdirAll(filepath.Join(root, ".kyo"), 0o700)
			_ = safeWriteAtomic(filepath.Join(root, ".kyo", "credentials"), []byte(res.Token+"\n"), 0o600)
		}
		return mcpText(fmt.Sprintf("Token renovado para Espacio %s; vence %s.", res.SpaceID, res.ExpiresAt), false)
	case "project_unlink":
		body, code, err := apiCall(http.MethodPost, "/project-links/unlink", ref)
		if err != nil || code >= 400 {
			return mcpText(fmt.Sprintf("Error desvinculando (%d): %v %s", code, err, string(body)), true)
		}
		_ = os.Remove(filepath.Join(root, projectFileName))
		_ = os.Remove(filepath.Join(root, ".kyo", "credentials"))
		return mcpText("Proyecto desvinculado. El Espacio y el Proyecto se conservan en Kyō.", false)
	default:
		return mcpText("tool desconocida: "+name, true)
	}
}

// ── Claude Code StatusLine ──────────────────────────────────────────────────

func runStatusLine(args []string) int {
	var wrapped string
	for i := 0; i < len(args); i++ {
		if args[i] == "--then" && i+1 < len(args) {
			wrapped = args[i+1]
			i++
		}
	}
	stdinBytes, _ := io.ReadAll(os.Stdin)
	cwd, _ := os.Getwd()
	var payload struct {
		Cwd       string `json:"cwd"`
		Workspace struct {
			CurrentDir string `json:"current_dir"`
		} `json:"workspace"`
	}
	if json.Unmarshal(stdinBytes, &payload) == nil {
		if payload.Workspace.CurrentDir != "" {
			cwd = payload.Workspace.CurrentDir
		} else if payload.Cwd != "" {
			cwd = payload.Cwd
		}
	}

	root := findProjectRootDir(cwd)
	mine := ""
	if readProjectFileLocal(root) != nil {
		color := os.Getenv("NO_COLOR") == ""
		paint := func(code, s string) string {
			if !color {
				return s
			}
			return "\u001b[" + code + "m" + s + "\u001b[0m"
		}
		_, err := loadAccountSession("")
		if err == nil {
			mine = paint("32", "●") + " " + paint("1", "Kyō") + " activo"
		} else {
			mine = paint("33", "○") + " " + paint("1", "Kyō") + " sin sesión · abre Kyo Desktop"
		}
	}

	theirs := ""
	if wrapped != "" {
		ctx, cancel := context.WithTimeout(context.Background(), 1500*time.Millisecond)
		defer cancel()
		var cmd *exec.Cmd
		if runtime.GOOS == "windows" {
			cmd = exec.CommandContext(ctx, "cmd", "/c", wrapped)
		} else {
			cmd = exec.CommandContext(ctx, "sh", "-c", wrapped)
		}
		cmd.Stdin = bytes.NewReader(stdinBytes)
		if out, err := cmd.Output(); err == nil {
			lines := strings.Split(strings.TrimRight(string(out), "\r\n "), "\n")
			if len(lines) > 0 {
				theirs = lines[0]
			}
		}
	}

	var parts []string
	if theirs != "" {
		parts = append(parts, theirs)
	}
	if mine != "" {
		parts = append(parts, mine)
	}
	if len(parts) > 0 {
		fmt.Println(strings.Join(parts, " │ "))
	}
	return 0
}

// ── Claude Code Auto-Log Stop Hook ──────────────────────────────────────────

func runHookStop() int {
	stdinBytes, err := io.ReadAll(os.Stdin)
	if err != nil || len(stdinBytes) == 0 {
		return 0
	}
	var input struct {
		Cwd            string `json:"cwd"`
		TranscriptPath string `json:"transcript_path"`
		StopHookActive bool   `json:"stop_hook_active"`
	}
	if json.Unmarshal(stdinBytes, &input) != nil || input.StopHookActive || input.TranscriptPath == "" {
		return 0
	}
	cwd := input.Cwd
	if cwd == "" {
		cwd, _ = os.Getwd()
	}
	root := findProjectRootDir(cwd)
	pf := readProjectFileLocal(root)
	if pf == nil || !pf.AutoLog {
		return 0
	}
	if isSymlinkPath(input.TranscriptPath) {
		return 0
	}
	transcriptBytes, err := os.ReadFile(input.TranscriptPath)
	if err != nil {
		return 0
	}

	pendingID := findPendingEditID(string(transcriptBytes), root)
	if pendingID == "" {
		return 0
	}

	stateFile := filepath.Join(root, ".kyo", "hook-state.json")
	if !isSymlinkPath(stateFile) {
		if raw, err := os.ReadFile(stateFile); err == nil {
			var st struct {
				Nudged string `json:"nudged"`
			}
			if json.Unmarshal(raw, &st) == nil && st.Nudged == pendingID {
				return 0
			}
		}
	}
	_ = os.MkdirAll(filepath.Join(root, ".kyo"), 0o700)
	stateBytes, _ := json.Marshal(map[string]string{"nudged": pendingID})
	if safeWriteAtomic(stateFile, append(stateBytes, '\n'), 0o600) != nil {
		return 0
	}

	const nudgeReason = "Hiciste cambios en el software durante esta sesión y no los registraste en el Espacio de Grozy. Antes de terminar: " +
		"(1) get_project del repo para ubicar el Espacio; (2) en su página \"Changelog\" (créala si falta) agrega arriba una entrada con fecha: qué cambió, por qué, archivos clave y cómo verificarlo, redactada desde `git diff`/`git log` reales; " +
		"(3) escapa < y > como &lt; &gt; y relee la página para verificar. Si el MCP `kyo` no responde, díselo al usuario en vez de omitirlo. " +
		"Si el cambio no es una implementación (sólo exploración o documentación), ignora este aviso."

	out, _ := json.Marshal(map[string]string{
		"decision": "block",
		"reason":   nudgeReason,
	})
	fmt.Println(string(out))
	return 0
}

var (
	reRegisterTool = regexp.MustCompile(`(?:^|__)(?:create_space_page|update_space_page|batch_update_space_pages)$`)
	reIgnoredEdit  = regexp.MustCompile(`(^|[\\/])(\.claude|\.kyo)[\\/]|(^|[\\/])(CLAUDE|AGENTS|GEMINI)\.md$|(^|[\\/])\.mcp\.json$|(^|[\\/])kyo-project\.json$`)
)

func findPendingEditID(transcript, root string) string {
	var pending string
	for _, line := range strings.Split(transcript, "\n") {
		if strings.TrimSpace(line) == "" {
			continue
		}
		var entry struct {
			Type    string `json:"type"`
			Message struct {
				Content json.RawMessage `json:"content"`
			} `json:"message"`
		}
		if json.Unmarshal([]byte(line), &entry) != nil || entry.Type != "assistant" {
			continue
		}
		var parts []struct {
			Type  string `json:"type"`
			ID    string `json:"id"`
			Name  string `json:"name"`
			Input struct {
				FilePath     string `json:"file_path"`
				NotebookPath string `json:"notebook_path"`
			} `json:"input"`
		}
		if json.Unmarshal(entry.Message.Content, &parts) != nil {
			continue
		}
		for _, part := range parts {
			if part.Type != "tool_use" || part.Name == "" {
				continue
			}
			if reRegisterTool.MatchString(part.Name) {
				pending = ""
			} else if part.Name == "Edit" || part.Name == "Write" || part.Name == "MultiEdit" || part.Name == "NotebookEdit" {
				target := part.Input.FilePath
				if target == "" {
					target = part.Input.NotebookPath
				}
				if !reIgnoredEdit.MatchString(target) && isInsideProjectRoot(target, root) {
					if part.ID != "" {
						pending = part.ID
					} else {
						pending = "sin-id"
					}
				}
			}
		}
	}
	return pending
}

func isInsideProjectRoot(file, root string) bool {
	if root == "" {
		return true
	}
	absTarget := file
	if !filepath.IsAbs(absTarget) {
		absTarget = filepath.Join(root, file)
	}
	rel, err := filepath.Rel(root, absTarget)
	if err != nil {
		return false
	}
	return rel != "" && rel != "." && !strings.HasPrefix(rel, "..") && !filepath.IsAbs(rel)
}
