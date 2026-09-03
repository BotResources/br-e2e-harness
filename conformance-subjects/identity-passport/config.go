package main

import (
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
	key, err := sealKeyFromEnv(osLookupEnv)
	if err != nil {
		return config{}, err
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
