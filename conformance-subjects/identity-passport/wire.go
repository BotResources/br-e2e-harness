package main

import (
	"bytes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	bearerPrefix          = "Bearer "
	bearerTokensKeyPrefix = "identity/bearer_tokens/"
)

type sealedBearer struct {
	Nonce      string `json:"nonce"`
	Ciphertext string `json:"ciphertext"`
}

type bearerActor struct {
	Kind string `json:"kind"`
	ID   string `json:"id"`
}

type bearerEntry struct {
	Actor   bearerActor `json:"actor"`
	TokenID string      `json:"token_id"`
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

func sha256Hex(token string) string {
	digest := sha256.Sum256([]byte(token))
	return hex.EncodeToString(digest[:])
}

func kvKey(token string) string {
	return bearerTokensKeyPrefix + sha256Hex(token)
}

func aad(token string) []byte {
	return []byte(sha256Hex(token))
}

func parseSealed(raw []byte) (sealedBearer, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	var sb sealedBearer
	if err := dec.Decode(&sb); err != nil {
		return sealedBearer{}, err
	}
	return sb, nil
}

func sealEntry(aead cipher.AEAD, token string, entry bearerEntry) (sealedBearer, error) {
	nonce := make([]byte, chacha20poly1305.NonceSize)
	if _, err := rand.Read(nonce); err != nil {
		return sealedBearer{}, fmt.Errorf("drawing a %d-byte nonce: %w", chacha20poly1305.NonceSize, err)
	}
	return sealEntryWithNonce(aead, token, entry, nonce)
}

func sealEntryWithNonce(aead cipher.AEAD, token string, entry bearerEntry, nonce []byte) (sealedBearer, error) {
	if len(nonce) != chacha20poly1305.NonceSize {
		return sealedBearer{}, fmt.Errorf("nonce must be %d bytes, got %d", chacha20poly1305.NonceSize, len(nonce))
	}
	plaintext, err := json.Marshal(entry)
	if err != nil {
		return sealedBearer{}, fmt.Errorf("marshalling the bearer entry: %w", err)
	}
	ciphertext := aead.Seal(nil, nonce, plaintext, aad(token))
	return sealedBearer{
		Nonce:      base64.StdEncoding.EncodeToString(nonce),
		Ciphertext: base64.StdEncoding.EncodeToString(ciphertext),
	}, nil
}

func openSealed(aead cipher.AEAD, token string, sb sealedBearer) (bearerEntry, error) {
	nonce, err := base64.StdEncoding.DecodeString(sb.Nonce)
	if err != nil {
		return bearerEntry{}, fmt.Errorf("nonce not base64-std: %w", err)
	}
	if len(nonce) != chacha20poly1305.NonceSize {
		return bearerEntry{}, fmt.Errorf("nonce must be %d bytes, got %d", chacha20poly1305.NonceSize, len(nonce))
	}
	ciphertext, err := base64.StdEncoding.DecodeString(sb.Ciphertext)
	if err != nil {
		return bearerEntry{}, fmt.Errorf("ciphertext not base64-std: %w", err)
	}
	plaintext, err := aead.Open(nil, nonce, ciphertext, aad(token))
	if err != nil {
		return bearerEntry{}, fmt.Errorf("aead open failed: %w", err)
	}
	dec := json.NewDecoder(bytes.NewReader(plaintext))
	dec.DisallowUnknownFields()
	var entry bearerEntry
	if err := dec.Decode(&entry); err != nil {
		return bearerEntry{}, fmt.Errorf("opened plaintext is not a bearer entry: %w", err)
	}
	return entry, nil
}

func passportForEntry(entry bearerEntry) humanPassport {
	return humanPassport{
		Kind:         "human",
		UserID:       entry.Actor.ID,
		IsSuperAdmin: false,
		IsActive:     true,
		AuthMethod:   authMethod{Method: "pat", TokenID: entry.TokenID},
		Impersonator: nil,
		Claims:       map[string]any{},
	}
}

func encodePassportHeader(passport humanPassport) (string, error) {
	body, err := json.Marshal(passport)
	if err != nil {
		return "", err
	}
	return base64.StdEncoding.EncodeToString(body), nil
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
