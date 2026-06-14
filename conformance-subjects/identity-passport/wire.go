package main

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"

	"github.com/google/uuid"
)

const bearerPrefix = "Bearer "

var userIDNamespace = uuid.MustParse("a7d4e2f0-3b91-4c6a-8f12-5e0c9d7b1a23")

type bearerTokenEntry struct {
	Email   string `json:"email"`
	TokenID string `json:"token_id"`
}

type authMethod struct {
	Method  string `json:"method"`
	TokenID string `json:"token_id"`
}

type humanPassport struct {
	Kind         string         `json:"kind"`
	UserID       string         `json:"user_id"`
	IsSuperAdmin bool           `json:"is_super_admin"`
	IsActive     bool           `json:"is_active"`
	AuthMethod   authMethod     `json:"auth_method"`
	Impersonator *string        `json:"impersonator"`
	Claims       map[string]any `json:"claims"`
}

func bearerTokenKey(token string) string {
	digest := sha256.Sum256([]byte(token))
	return hex.EncodeToString(digest[:])
}

func userIDFromEmail(email string) string {
	return uuid.NewSHA1(userIDNamespace, []byte(email)).String()
}

func passportForEntry(entry bearerTokenEntry) humanPassport {
	return humanPassport{
		Kind:         "human",
		UserID:       userIDFromEmail(entry.Email),
		IsSuperAdmin: false,
		IsActive:     true,
		AuthMethod:   authMethod{Method: "pat", TokenID: entry.TokenID},
		Impersonator: nil,
		Claims:       map[string]any{"email": entry.Email},
	}
}

func encodePassportHeader(passport humanPassport) (string, error) {
	body, err := json.Marshal(passport)
	if err != nil {
		return "", err
	}
	return base64.StdEncoding.EncodeToString(body), nil
}
