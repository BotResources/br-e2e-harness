package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"net/http"
	"os/signal"
	"syscall"
	"time"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
	"golang.org/x/crypto/chacha20poly1305"
)

const publishedLanguageBucket = "PUBLISHED_LANGUAGE"

func main() {
	if err := run(); err != nil {
		log.Fatalf("fatal: %v", err)
	}
}

func run() error {
	cfg, err := loadConfig()
	if err != nil {
		return err
	}

	aead, err := chacha20poly1305.New(cfg.sealKey)
	if err != nil {
		return fmt.Errorf("building the chacha20-poly1305 aead from BEARER_SEAL_KEY: %w", err)
	}

	ready := newReadiness("binding the PUBLISHED_LANGUAGE bucket")
	res := newResolver(aead)

	addr := "0.0.0.0:" + cfg.port
	mux := http.NewServeMux()
	mux.HandleFunc("/internal/passport", res.handle)
	mux.HandleFunc("/readyz", ready.readyzHandler)
	mux.HandleFunc("/livez", livezHandler)
	server := &http.Server{
		Addr:              addr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		log.Printf("http listening on %s (/internal/passport, /readyz, /livez)", addr)
		if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("http server: %v", err)
		}
	}()

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	nc, err := nats.Connect(cfg.natsURL,
		nats.RetryOnFailedConnect(true),
		nats.MaxReconnects(-1),
		nats.ReconnectWait(time.Second),
	)
	if err != nil {
		return err
	}
	defer nc.Drain()

	js, err := jetstream.New(nc)
	if err != nil {
		return err
	}

	bindCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	kv, err := js.KeyValue(bindCtx, publishedLanguageBucket)
	if err != nil {
		ready.setNotReady(fmt.Sprintf("PUBLISHED_LANGUAGE bucket unreachable: %v", err))
		return fmt.Errorf("bind bucket %q (it must exist — the operator/Helm provisions it, the subject never does): %w", publishedLanguageBucket, err)
	}

	res.bind(kv)
	ready.setReady()
	log.Printf("bound bucket %s; readiness UP", publishedLanguageBucket)

	<-ctx.Done()
	log.Printf("shutdown signal received")

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer shutdownCancel()
	return server.Shutdown(shutdownCtx)
}
