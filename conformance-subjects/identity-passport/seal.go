package main

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"strings"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	tamperNone       = ""
	tamperCiphertext = "ciphertext"
	tamperNonce      = "nonce"
)

type sealRequest struct {
	keyB64     string
	token      string
	actor      string
	tokenID    string
	tamper     string
	unreadable bool
}

type sealResult struct {
	KvKey    string `json:"kv_key"`
	ValueB64 string `json:"value_b64"`
}

func runSeal(args []string, out io.Writer) error {
	req, err := parseSealArgs(args)
	if err != nil {
		return err
	}
	result, err := sealOnce(req)
	if err != nil {
		return err
	}
	line, err := json.Marshal(result)
	if err != nil {
		return fmt.Errorf("marshalling the seal result: %w", err)
	}
	_, err = fmt.Fprintf(out, "%s\n", line)
	return err
}

func parseSealArgs(args []string) (sealRequest, error) {
	fs := flag.NewFlagSet("seal", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	var req sealRequest
	fs.StringVar(&req.keyB64, "key", "", "base64-std of the 32-byte seal key")
	fs.StringVar(&req.token, "token", "", "the raw bearer token")
	fs.StringVar(&req.actor, "actor", "", "human:<uuid> or service:<uuid>")
	fs.StringVar(&req.tokenID, "token-id", "", "the token id (uuid)")
	fs.StringVar(&req.tamper, "tamper", tamperNone, "flip the first byte of ciphertext|nonce after sealing")
	fs.BoolVar(&req.unreadable, "unreadable", false, "emit an envelope the parser must reject")
	if err := fs.Parse(args); err != nil {
		return sealRequest{}, err
	}
	if fs.NArg() != 0 {
		return sealRequest{}, fmt.Errorf("unexpected positional argument %q", fs.Arg(0))
	}
	return req, validateSealRequest(req)
}

func validateSealRequest(req sealRequest) error {
	if req.keyB64 == "" {
		return errors.New("--key is required (base64-std of 32 bytes)")
	}
	if req.token == "" {
		return errors.New("--token is required")
	}
	if req.actor == "" {
		return errors.New("--actor is required (human:<uuid> or service:<uuid>)")
	}
	if req.tokenID == "" {
		return errors.New("--token-id is required")
	}
	if !isUUID(req.tokenID) {
		return fmt.Errorf("--token-id %q is not a uuid", req.tokenID)
	}
	if _, err := parseActor(req.actor); err != nil {
		return err
	}
	switch req.tamper {
	case tamperNone, tamperCiphertext, tamperNonce:
	default:
		return fmt.Errorf("--tamper must be %q or %q, got %q", tamperCiphertext, tamperNonce, req.tamper)
	}
	if req.unreadable && req.tamper != tamperNone {
		return errors.New("--unreadable and --tamper are mutually exclusive")
	}
	return nil
}

func parseActor(raw string) (bearerActor, error) {
	kind, id, ok := strings.Cut(raw, ":")
	if !ok {
		return bearerActor{}, fmt.Errorf("--actor %q must be kind:uuid", raw)
	}
	if kind != "human" && kind != "service" {
		return bearerActor{}, fmt.Errorf("--actor kind %q must be human or service", kind)
	}
	if !isUUID(id) {
		return bearerActor{}, fmt.Errorf("--actor id %q is not a uuid", id)
	}
	return bearerActor{Kind: kind, ID: id}, nil
}

func isUUID(value string) bool {
	groups := []int{8, 4, 4, 4, 12}
	parts := strings.Split(value, "-")
	if len(parts) != len(groups) {
		return false
	}
	for i, part := range parts {
		if len(part) != groups[i] {
			return false
		}
		for _, c := range part {
			isHex := (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
			if !isHex {
				return false
			}
		}
	}
	return true
}

func sealOnce(req sealRequest) (sealResult, error) {
	key, err := base64.StdEncoding.DecodeString(req.keyB64)
	if err != nil {
		return sealResult{}, fmt.Errorf("--key is not valid base64-std: %w", err)
	}
	if len(key) != bearerSealKeyLen {
		return sealResult{}, fmt.Errorf("--key must decode to %d bytes, got %d", bearerSealKeyLen, len(key))
	}
	aead, err := chacha20poly1305.New(key)
	if err != nil {
		return sealResult{}, fmt.Errorf("building the aead: %w", err)
	}
	actor, err := parseActor(req.actor)
	if err != nil {
		return sealResult{}, err
	}
	sealed, err := sealEntry(aead, req.token, bearerEntry{Actor: actor, TokenID: req.tokenID})
	if err != nil {
		return sealResult{}, err
	}
	value, err := renderStoredValue(sealed, req)
	if err != nil {
		return sealResult{}, err
	}
	return sealResult{
		KvKey:    kvKey(req.token),
		ValueB64: base64.StdEncoding.EncodeToString(value),
	}, nil
}

func renderStoredValue(sealed sealedBearer, req sealRequest) ([]byte, error) {
	if req.unreadable {
		return json.Marshal(map[string]any{
			"nonce":      sealed.Nonce,
			"ciphertext": sealed.Ciphertext,
			"evil":       true,
		})
	}
	tampered, err := applyTamper(sealed, req.tamper)
	if err != nil {
		return nil, err
	}
	return json.Marshal(tampered)
}

func applyTamper(sealed sealedBearer, tamper string) (sealedBearer, error) {
	switch tamper {
	case tamperNone:
		return sealed, nil
	case tamperCiphertext:
		flipped, err := flipFirstByte(sealed.Ciphertext)
		if err != nil {
			return sealedBearer{}, fmt.Errorf("tampering the ciphertext: %w", err)
		}
		sealed.Ciphertext = flipped
		return sealed, nil
	case tamperNonce:
		flipped, err := flipFirstByte(sealed.Nonce)
		if err != nil {
			return sealedBearer{}, fmt.Errorf("tampering the nonce: %w", err)
		}
		sealed.Nonce = flipped
		return sealed, nil
	default:
		return sealedBearer{}, fmt.Errorf("unknown tamper mode %q", tamper)
	}
}

func flipFirstByte(b64 string) (string, error) {
	raw, err := base64.StdEncoding.DecodeString(b64)
	if err != nil {
		return "", err
	}
	if len(raw) == 0 {
		return "", errors.New("nothing to flip: the decoded value is empty")
	}
	raw[0] ^= 0xff
	return base64.StdEncoding.EncodeToString(raw), nil
}
