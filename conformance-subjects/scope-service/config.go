package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

type config struct {
	natsURL  string
	httpAddr string

	serviceKey     string
	scopeKeys      []string
	labelKey       string
	descriptionKey string
	platformOnly   bool

	waitTimeout time.Duration
	enabled     bool
}

func loadConfig() (config, error) {
	c := config{
		natsURL:        getenv("NATS_URL", "nats://127.0.0.1:4222"),
		httpAddr:       getenv("HTTP_ADDR", ":8080"),
		serviceKey:     os.Getenv("SERVICE_KEY"),
		labelKey:       getenv("LABEL_KEY", ""),
		descriptionKey: getenv("DESCRIPTION_KEY", ""),
	}

	c.scopeKeys = splitCSV(os.Getenv("SCOPE_KEYS"))

	platformOnly, err := parseBool("PLATFORM_ONLY", false)
	if err != nil {
		return config{}, err
	}
	c.platformOnly = platformOnly

	enabled, err := parseBool("SCOPE_DECLARATION_ENABLED", true)
	if err != nil {
		return config{}, err
	}
	c.enabled = enabled

	timeout, err := parseDuration("WAIT_TIMEOUT", 10*time.Second)
	if err != nil {
		return config{}, err
	}
	c.waitTimeout = timeout

	if c.serviceKey == "" {
		return config{}, fmt.Errorf("SERVICE_KEY is required")
	}

	return c, nil
}

func (c config) declaration() declareServiceScopes {
	scopes := make([]rawScopeSpec, 0, len(c.scopeKeys))
	for _, key := range c.scopeKeys {
		scopes = append(scopes, rawScopeSpec{
			Key:            key,
			LabelKey:       c.labelKey,
			DescriptionKey: c.descriptionKey,
			PlatformOnly:   c.platformOnly,
		})
	}
	return declareServiceScopes{
		Declaration: rawScopeDeclaration{
			Manifest: rawServiceManifest{
				Key:            c.serviceKey,
				LabelKey:       c.labelKey,
				DescriptionKey: c.descriptionKey,
			},
			Scopes: scopes,
		},
	}
}

func getenv(key, fallback string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return fallback
}

func splitCSV(raw string) []string {
	if strings.TrimSpace(raw) == "" {
		return nil
	}
	parts := strings.Split(raw, ",")
	out := make([]string, 0, len(parts))
	for _, p := range parts {
		if trimmed := strings.TrimSpace(p); trimmed != "" {
			out = append(out, trimmed)
		}
	}
	return out
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

func parseDuration(key string, fallback time.Duration) (time.Duration, error) {
	raw, ok := os.LookupEnv(key)
	if !ok || strings.TrimSpace(raw) == "" {
		return fallback, nil
	}
	v, err := time.ParseDuration(strings.TrimSpace(raw))
	if err != nil {
		return 0, fmt.Errorf("%s: %w", key, err)
	}
	return v, nil
}
