package main

import (
	"context"
	"crypto/cipher"
	"errors"
	"log"
	"net/http"
	"sync/atomic"

	"github.com/nats-io/nats.go/jetstream"
)

type resolver struct {
	kv   atomic.Pointer[jetstream.KeyValue]
	aead cipher.AEAD
}

func newResolver(aead cipher.AEAD) *resolver {
	return &resolver{aead: aead}
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

	raw, found, err := r.lookup(req.Context(), *kvPtr, token)
	if err != nil {
		log.Printf("bearer kv lookup failed: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	if !found {
		w.WriteHeader(http.StatusOK)
		return
	}

	sealed, err := parseSealed(raw)
	if err != nil {
		log.Printf("stored sealed bearer is unreadable, resolving anonymous: %v", err)
		w.WriteHeader(http.StatusOK)
		return
	}

	entry, err := openSealed(r.aead, token, sealed)
	if err != nil {
		log.Printf("sealed bearer did not open (wrong key or tampered), resolving anonymous: %v", err)
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

func (r *resolver) lookup(ctx context.Context, kv jetstream.KeyValue, token string) ([]byte, bool, error) {
	value, err := kv.Get(ctx, kvKey(token))
	if errors.Is(err, jetstream.ErrKeyNotFound) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, err
	}
	return value.Value(), true, nil
}
