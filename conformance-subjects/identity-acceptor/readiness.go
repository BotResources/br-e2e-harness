package main

import (
	"net/http"
	"sync"
)

type readiness struct {
	mu     sync.RWMutex
	ready  bool
	reason string
}

func newReadiness(reason string) *readiness {
	return &readiness{ready: false, reason: reason}
}

func (r *readiness) setReady() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.ready = true
	r.reason = ""
}

func (r *readiness) setNotReady(reason string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.ready = false
	r.reason = reason
}

func (r *readiness) snapshot() (bool, string) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.ready, r.reason
}

func (r *readiness) readyzHandler(w http.ResponseWriter, _ *http.Request) {
	ready, reason := r.snapshot()
	if ready {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ready"))
		return
	}
	w.WriteHeader(http.StatusServiceUnavailable)
	if reason != "" {
		_, _ = w.Write([]byte(reason))
		return
	}
	_, _ = w.Write([]byte("not ready"))
}

func livezHandler(w http.ResponseWriter, _ *http.Request) {
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte("alive"))
}
