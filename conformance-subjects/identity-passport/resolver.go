package main

import (
	"context"
	"encoding/json"
	"errors"
	"log"
	"net/http"
	"strings"
	"sync/atomic"

	"github.com/nats-io/nats.go/jetstream"
)

type resolver struct {
	kv  atomic.Pointer[jetstream.KeyValue]
	cfg config
}

func newResolver(cfg config) *resolver {
	return &resolver{cfg: cfg}
}

func (r *resolver) bind(kv jetstream.KeyValue) {
	r.kv.Store(&kv)
}

func (r *resolver) handle(w http.ResponseWriter, req *http.Request) {
	kvPtr := r.kv.Load()
	if kvPtr == nil {
		w.WriteHeader(http.StatusServiceUnavailable)
		return
	}

	token, ok := bearerToken(req.Header.Get("Authorization"))
	if !ok {
		w.WriteHeader(http.StatusOK)
		return
	}

	entry, found, err := r.lookup(req.Context(), *kvPtr, token)
	if err != nil {
		log.Printf("bearer lookup failed: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	if !found {
		w.WriteHeader(http.StatusOK)
		return
	}

	header, err := encodePassportHeader(passportForEntry(entry))
	if err != nil {
		log.Printf("passport encode failed: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	w.Header().Set("X-Passport", header)
	w.WriteHeader(http.StatusOK)
}

func (r *resolver) lookup(ctx context.Context, kv jetstream.KeyValue, token string) (bearerTokenEntry, bool, error) {
	value, err := kv.Get(ctx, bearerTokenKey(token))
	if errors.Is(err, jetstream.ErrKeyNotFound) {
		return bearerTokenEntry{}, false, nil
	}
	if err != nil {
		return bearerTokenEntry{}, false, err
	}
	var entry bearerTokenEntry
	if err := json.Unmarshal(value.Value(), &entry); err != nil {
		return bearerTokenEntry{}, false, err
	}
	return entry, true, nil
}

func bearerToken(authorization string) (string, bool) {
	if len(authorization) <= len(bearerPrefix) {
		return "", false
	}
	if !strings.HasPrefix(authorization, bearerPrefix) {
		return "", false
	}
	token := authorization[len(bearerPrefix):]
	if token == "" {
		return "", false
	}
	return token, true
}
