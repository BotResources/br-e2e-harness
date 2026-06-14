package main

import (
	"os"
)

type config struct {
	natsURL      string
	httpAddr     string
	bearerBucket string
}

func loadConfig() (config, error) {
	c := config{
		natsURL:      getenv("NATS_URL", "nats://127.0.0.1:4222"),
		httpAddr:     getenv("HTTP_ADDR", ":8080"),
		bearerBucket: getenv("BEARER_BUCKET", "bearer_tokens"),
	}
	return c, nil
}

func getenv(key, fallback string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return fallback
}
