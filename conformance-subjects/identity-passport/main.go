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
)

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

	ready := newReadiness("binding the bearer_tokens bucket")
	res := newResolver(cfg)

	mux := http.NewServeMux()
	mux.HandleFunc("/internal/passport", res.handle)
	mux.HandleFunc("/readyz", ready.readyzHandler)
	mux.HandleFunc("/livez", livezHandler)
	server := &http.Server{
		Addr:              cfg.httpAddr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		log.Printf("http listening on %s (/internal/passport, /readyz, /livez)", cfg.httpAddr)
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
	kv, err := js.KeyValue(bindCtx, cfg.bearerBucket)
	if err != nil {
		ready.setNotReady(fmt.Sprintf("bearer_tokens bucket %q unreachable: %v", cfg.bearerBucket, err))
		return fmt.Errorf("bind bucket %q (it must exist — the subject never provisions it): %w", cfg.bearerBucket, err)
	}

	res.bind(kv)
	ready.setReady()
	log.Printf("bound bucket %s; readiness UP", cfg.bearerBucket)

	<-ctx.Done()
	log.Printf("shutdown signal received")

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer shutdownCancel()
	return server.Shutdown(shutdownCtx)
}
