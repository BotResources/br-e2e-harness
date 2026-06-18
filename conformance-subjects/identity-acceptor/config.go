package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

type config struct {
	natsURL  string
	httpAddr string
	enabled  bool
}

func loadConfig() (config, error) {
	c := config{
		natsURL:  getenv("NATS_URL", "nats://127.0.0.1:4222"),
		httpAddr: getenv("HTTP_ADDR", ":8080"),
	}

	enabled, err := parseBool("SCOPE_ACCEPTANCE_ENABLED", true)
	if err != nil {
		return config{}, err
	}
	c.enabled = enabled

	return c, nil
}

func getenv(key, fallback string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return fallback
}

func parseBool(key string, fallback bool) (bool, error) {
	raw, ok := os.LookupEnv(key)
	if !ok || strings.TrimSpace(raw) == "" {
		return fallback, nil
	}
	v, err := strconv.ParseBool(strings.TrimSpace(raw))
	if err != nil {
		return false, fmt.Errorf("%s: %w", key, err)
	}
	return v, nil
}
