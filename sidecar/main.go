package main

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
)

type HealthResponse struct {
	Status   string `json:"status"`
	Mode     string `json:"mode"`
	Port     int    `json:"port"`
	Upstream string `json:"upstream,omitempty"`
	DBDir    string `json:"db_dir,omitempty"`
}

func corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusOK)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func main() {
	portStr := os.Getenv("PORT")
	if portStr == "" {
		portStr = "0"
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

	// Enlazar a 127.0.0.1 en el puerto configurado (o 0 para efímero)
	listener, err := net.Listen("tcp", "127.0.0.1:"+portStr)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error al iniciar listener TCP en 127.0.0.1:%s: %v\n", portStr, err)
		os.Exit(1)
	}

	actualPort := listener.Addr().(*net.TCPAddr).Port

	mux := http.NewServeMux()

	// Endpoints base requeridos por el handshake y health check
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(HealthResponse{
			Status: "ok",
			Mode:   mode,
			Port:   actualPort,
		})
	})

	mux.HandleFunc("/api/v1/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(HealthResponse{
			Status:   "ok",
			Mode:     mode,
			Port:     actualPort,
			Upstream: upstream,
			DBDir:    dbDir,
		})
	})

	mux.HandleFunc("/api/v1/ping", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"message": "pong"})
	})

	// Handshake crítico para Rust/Tauri en stdout
	fmt.Printf("KYO_SIDECAR_READY port=%d upstream=%s db=%s\n", actualPort, upstream, dbDir)
	_ = os.Stdout.Sync()

	server := &http.Server{
		Handler: corsMiddleware(mux),
	}

	if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
		fmt.Fprintf(os.Stderr, "Error en el servidor HTTP: %v\n", err)
		os.Exit(1)
	}
}
