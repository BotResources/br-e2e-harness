package main

import (
	"encoding/base64"
	"fmt"
	"os"
)

const bearerSealKeyLen = 32

type config struct {
	natsURL string
	port    string
	sealKey []byte
}

func loadConfig() (config, error) {
	port, ok := os.LookupEnv("PORT")
	if !ok || port == "" {
		return config{}, fmt.Errorf("PORT is required (the loopback port to bind)")
	}
	rawKey, ok := os.LookupEnv("BEARER_SEAL_KEY")
	if !ok || rawKey == "" {
		return config{}, fmt.Errorf("BEARER_SEAL_KEY is required (base64-std of a 32-byte key)")
	}
	key, err := base64.StdEncoding.DecodeString(rawKey)
	if err != nil {
		return config{}, fmt.Errorf("BEARER_SEAL_KEY is not valid base64-std: %w", err)
	}
	if len(key) != bearerSealKeyLen {
		return config{}, fmt.Errorf("BEARER_SEAL_KEY must decode to %d bytes, got %d", bearerSealKeyLen, len(key))
	}
	return config{
		natsURL: getenv("NATS_URL", "nats://127.0.0.1:4222"),
		port:    port,
		sealKey: key,
	}, nil
}

func getenv(key, fallback string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return fallback
}
