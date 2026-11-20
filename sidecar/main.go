package main

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

type HealthResponse struct {
	Status                string `json:"status"`
	Mode                  string `json:"mode"`
	Port                  int    `json:"port"`
	Upstream              string `json:"upstream,omitempty"`
	DBDir                 string `json:"db_dir,omitempty"`
	HasLocalVault         bool   `json:"has_local_vault"`
	LocalVaultModeEnabled bool   `json:"local_vault_mode_enabled"`
	LocalVaultsCount      int    `json:"local_vaults_count"`
}

type LocalVaultEntry struct {
	ID          string  `json:"id"`
	Name        string  `json:"name"`
	Description *string `json:"description,omitempty"`
	Color       string  `json:"color"`
	RootPath    string  `json:"root_path"`
	CreatedAt   string  `json:"created_at"`
	UpdatedAt   string  `json:"updated_at"`
}

func loadLocalVaultEntries() []LocalVaultEntry {
	appData := os.Getenv("APPDATA")
	if appData == "" {
		return nil
	}
	regPath := filepath.Join(appData, "kyo", "local_vaults.json")
	raw, err := os.ReadFile(regPath)
	if err != nil {
		return nil
	}
	var list []LocalVaultEntry
	if err := json.Unmarshal(raw, &list); err != nil {
		return nil
	}
	return list
}

// watchParentProcess termina el sidecar si el proceso padre (Tauri/Dioxus) muere abruptamente
func watchParentProcess(cancel context.CancelFunc) {
	_, _ = io.Copy(io.Discard, os.Stdin)
	log.Println("Proceso padre desconectado (EOF en Stdin). Cerrando sidecar...")
	cancel()
	os.Exit(0)
}

// mcpSecurityMiddleware replica las defensas de src/lib/mcpSecurity.ts en el backend
func mcpSecurityMiddleware(next http.Handler, allowedToken string) http.Handler {
	allowedBytes := []byte(allowedToken)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Cabeceras de endurecimiento de seguridad
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Cache-Control", "no-store")

		// 1. Solo permitir conexiones loopback
		host, _, err := net.SplitHostPort(r.RemoteAddr)
		if err != nil || (host != "127.0.0.1" && host != "::1" && host != "localhost") {
			http.Error(w, "Forbidden: Localhost only", http.StatusForbidden)
			return
		}

		// 2. Validar token de sesión IPC en tiempo constante (previene ataques de timing side-channel)
		if len(allowedBytes) > 0 {
			provided := []byte(r.Header.Get("X-Kyo-Token"))
			if subtle.ConstantTimeCompare(provided, allowedBytes) != 1 {
				http.Error(w, "Unauthorized IPC Token", http.StatusUnauthorized)
				return
			}
		}

		// 3. Bloquear intentos de Path Traversal en la URL
		cleanPath := filepath.Clean(r.URL.Path)
		if strings.Contains(cleanPath, "..") {
			http.Error(w, "Bad Request: Invalid Path", http.StatusBadRequest)
			return
		}

		next.ServeHTTP(w, r)
	})
}

func corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		origin := r.Header.Get("Origin")

		allowedOrigins := map[string]bool{
			"http://tauri.localhost":  true,
			"https://tauri.localhost": true,
			"http://localhost":        true,
			"http://localhost:8080":   true,
			"https://kyo.broco.dev":   true,
		}

		if allowedOrigins[origin] {
			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Add("Vary", "Origin")
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Kyo-Token")
		}

		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusOK)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func main() {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Vigilar cierre de Tauri por Stdin
	go watchParentProcess(cancel)

	// Puerto configurable o dinámico (:0)
	port := os.Getenv("KYO_PORT")
	if port == "" {
		port = os.Getenv("PORT")
	}
	if port == "" {
		port = "0" // Asigna un puerto libre automáticamente
	}

	mode := os.Getenv("KYO_MODE")
	if mode == "" {
		mode = "local"
	}
	upstream := os.Getenv("KYO_UPSTREAM")
	if upstream == "" {
		upstream = "https://kyo.broco.dev"
	}
	dbDir := os.Getenv("KYO_DB_DIR")
	if dbDir == "" {
		dbDir = "replicas"
	}

	listener, err := net.Listen("tcp", "127.0.0.1:"+port)
	if err != nil {
		log.Fatalf("Error al abrir puerto: %v", err)
	}

	actualPort := listener.Addr().(*net.TCPAddr).Port
	// Línea crítica que captura src-tauri/src/sidecar.rs
	fmt.Printf("KYO_SIDECAR_PORT=%d\n", actualPort)
	// Handshake complementario para compatibilidad completa
	fmt.Printf("KYO_SIDECAR_READY port=%d upstream=%s db=%s\n", actualPort, upstream, dbDir)
	_ = os.Stdout.Sync()

	mux := http.NewServeMux()

	// Endpoints base requeridos por el handshake y health check
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		vaults := loadLocalVaultEntries()
		hasLocal := len(vaults) > 0
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(HealthResponse{
			Status:                "ok",
			Mode:                  mode,
			Port:                  actualPort,
			HasLocalVault:         hasLocal,
			LocalVaultModeEnabled: hasLocal,
			LocalVaultsCount:      len(vaults),
		})
	})

	mux.HandleFunc("/api/v1/health", func(w http.ResponseWriter, r *http.Request) {
		vaults := loadLocalVaultEntries()
		hasLocal := len(vaults) > 0
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(HealthResponse{
			Status:                "ok",
			Mode:                  mode,
			Port:                  actualPort,
			Upstream:              upstream,
			DBDir:                 dbDir,
			HasLocalVault:         hasLocal,
			LocalVaultModeEnabled: hasLocal,
			LocalVaultsCount:      len(vaults),
		})
	})

	// Endpoint para que el modo Web consulte si existe al menos una bóveda local creada
	mux.HandleFunc("/api/v1/local-vaults/status", func(w http.ResponseWriter, r *http.Request) {
		vaults := loadLocalVaultEntries()
		hasLocal := len(vaults) > 0
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"has_local_vault":          hasLocal,
			"local_vault_mode_enabled": hasLocal,
			"local_vaults_count":       len(vaults),
		})
	})

	// Endpoint para listar las bóvedas locales en el Modo Bóveda Local (solo devuelve bóvedas si hay >= 1 creada)
	mux.HandleFunc("/api/v1/local-vaults", func(w http.ResponseWriter, r *http.Request) {
		vaults := loadLocalVaultEntries()
		hasLocal := len(vaults) > 0
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"has_local_vault":          hasLocal,
			"local_vault_mode_enabled": hasLocal,
			"total":                    len(vaults),
			"data":                     vaults,
		})
	})

	mux.HandleFunc("/api/v1/ping", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"message": "pong"})
	})

	ipcToken := os.Getenv("KYO_IPC_TOKEN")
	server := &http.Server{
		Handler:           corsMiddleware(mcpSecurityMiddleware(mux, ipcToken)),
		ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout:       15 * time.Second,
		IdleTimeout:       60 * time.Second,
		BaseContext: func(l net.Listener) context.Context {
			return ctx
		},
	}

	// Manejo de señales del sistema operativo
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-sigChan
		cancel()
		_ = server.Shutdown(context.Background())
	}()

	if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
		log.Fatalf("Error en servidor sidecar: %v", err)
	}
}
