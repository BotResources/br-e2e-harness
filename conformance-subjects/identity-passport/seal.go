package main

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	tamperNone        = ""
	tamperCiphertext  = "ciphertext"
	tamperNonce       = "nonce"
	corruptUnreadable = "unreadable"
)

type sealRequest struct {
	key        []byte
	token      string
	actor      string
	tokenID    string
	tamper     string
	unreadable bool
	nonce      []byte
}

type sealResult struct {
	KvKey    string `json:"kv_key"`
	ValueB64 string `json:"value_b64"`
}

func runSeal(args []string, lookupEnv func(string) (string, bool), out io.Writer) error {
	req, err := parseSealArgs(args, lookupEnv)
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

func parseSealArgs(args []string, lookupEnv func(string) (string, bool)) (sealRequest, error) {
	fs := flag.NewFlagSet("seal", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	var req sealRequest
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
	key, err := sealKeyFromEnv(lookupEnv)
	if err != nil {
		return sealRequest{}, err
	}
	req.key = key
	return req, validateSealRequest(req)
}

func sealKeyFromEnv(lookupEnv func(string) (string, bool)) ([]byte, error) {
	raw, ok := lookupEnv("BEARER_SEAL_KEY")
	if !ok || raw == "" {
		return nil, errors.New("BEARER_SEAL_KEY is required (base64-std of a 32-byte key)")
	}
	return decodeSealKey(raw)
}

func decodeSealKey(raw string) ([]byte, error) {
	key, err := base64.StdEncoding.DecodeString(raw)
	if err != nil {
		return nil, fmt.Errorf("BEARER_SEAL_KEY is not valid base64-std: %w", err)
	}
	if len(key) != bearerSealKeyLen {
		return nil, fmt.Errorf("BEARER_SEAL_KEY must decode to %d bytes, got %d", bearerSealKeyLen, len(key))
	}
	return key, nil
}

func validateSealRequest(req sealRequest) error {
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
	if kind != actorHuman && kind != actorService {
		return bearerActor{}, fmt.Errorf("--actor kind %q must be %s or %s", kind, actorHuman, actorService)
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
	if len(req.key) != bearerSealKeyLen {
		return sealResult{}, fmt.Errorf("the seal key must be %d bytes, got %d", bearerSealKeyLen, len(req.key))
	}
	aead, err := chacha20poly1305.New(req.key)
	if err != nil {
		return sealResult{}, fmt.Errorf("building the aead: %w", err)
	}
	actor, err := parseActor(req.actor)
	if err != nil {
		return sealResult{}, err
	}
	entry := bearerEntry{Actor: actor, TokenID: req.tokenID}

	var sealed sealedBearer
	if req.nonce == nil {
		sealed, err = sealEntry(aead, req.token, entry)
	} else {
		sealed, err = sealEntryWithNonce(aead, req.token, entry, req.nonce)
	}
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
	faithful, err := json.Marshal(sealed)
	if err != nil {
		return nil, fmt.Errorf("marshalling the sealed envelope: %w", err)
	}
	mutation := req.tamper
	if req.unreadable {
		mutation = corruptUnreadable
	}
	if mutation == mutationNone {
		return faithful, nil
	}
	return mutateStoredValue(faithful, mutation)
}

func mutateStoredValue(faithful []byte, mutation string) ([]byte, error) {
	sealed, err := parseSealed(faithful)
	if err != nil {
		return nil, fmt.Errorf("the envelope to mutate must parse faithfully first: %w", err)
	}
	switch mutation {
	case corruptUnreadable:
		return json.Marshal(map[string]any{
			"nonce":      sealed.Nonce,
			"ciphertext": sealed.Ciphertext,
			"evil":       true,
		})
	case tamperCiphertext, tamperNonce:
		tampered, err := applyTamper(sealed, mutation)
		if err != nil {
			return nil, err
		}
		return json.Marshal(tampered)
	default:
		return nil, fmt.Errorf("unknown mutation %q", mutation)
	}
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

func osLookupEnv(key string) (string, bool) {
	return os.LookupEnv(key)
}
