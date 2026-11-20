package main

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestMcpSecurityMiddleware_LocalhostOnly(t *testing.T) {
	dummyHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	handler := mcpSecurityMiddleware(dummyHandler, "")

	tests := []struct {
		name       string
		remoteAddr string
		wantStatus int
	}{
		{"IPv4 Loopback", "127.0.0.1:12345", http.StatusOK},
		{"IPv6 Loopback", "[::1]:12345", http.StatusOK},
		{"External IP", "192.168.1.50:12345", http.StatusForbidden},
		{"Public IP", "8.8.8.8:12345", http.StatusForbidden},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest("GET", "/health", nil)
			req.RemoteAddr = tt.remoteAddr
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)
			if rec.Code != tt.wantStatus {
				t.Errorf("got status %d, want %d", rec.Code, tt.wantStatus)
			}
		})
	}
}

func TestMcpSecurityMiddleware_TokenValidation(t *testing.T) {
	dummyHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	handler := mcpSecurityMiddleware(dummyHandler, "secret-token-xyz")

	tests := []struct {
		name       string
		headerVal  string
		wantStatus int
	}{
		{"Correct Token", "secret-token-xyz", http.StatusOK},
		{"Wrong Token", "invalid-token", http.StatusUnauthorized},
		{"Missing Token", "", http.StatusUnauthorized},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest("GET", "/health", nil)
			req.RemoteAddr = "127.0.0.1:12345"
			if tt.headerVal != "" {
				req.Header.Set("X-Kyo-Token", tt.headerVal)
			}
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)
			if rec.Code != tt.wantStatus {
				t.Errorf("got status %d, want %d", rec.Code, tt.wantStatus)
			}
		})
	}
}

func TestMcpSecurityMiddleware_PathTraversal(t *testing.T) {
	dummyHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	handler := mcpSecurityMiddleware(dummyHandler, "")

	tests := []struct {
		name       string
		path       string
		wantStatus int
	}{
		{"Safe Path", "/health", http.StatusOK},
		{"Safe API Path", "/api/v1/ping", http.StatusOK},
		{"Path Traversal 1", "../secret", http.StatusBadRequest},
		{"Path Traversal 2", "../../etc/passwd", http.StatusBadRequest},
		{"Path Traversal 3", "foo/../../bar", http.StatusBadRequest},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest("GET", "/test", nil)
			req.URL.Path = tt.path
			req.RemoteAddr = "127.0.0.1:12345"
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)
			if rec.Code != tt.wantStatus {
				t.Errorf("got status %d, want %d", rec.Code, tt.wantStatus)
			}
		})
	}
}
