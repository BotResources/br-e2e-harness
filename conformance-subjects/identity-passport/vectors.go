package main

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	actorHuman         = "human"
	actorService       = "service"
	sealedWithSealKey  = "seal_key"
	sealedWithWrongKey = "wrong_seal_key"
	corruptionNone     = "none"
	resolvesHuman      = "human"
	resolvesAnonymous  = "anonymous"
	wireVectorsVersion = 1
)

var (
	frozenSealKey = []byte{
		0x1f, 0x2e, 0x3d, 0x4c, 0x5b, 0x6a, 0x79, 0x88, 0x97, 0xa6, 0xb5, 0xc4, 0xd3, 0xe2, 0xf1, 0x00,
		0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a, 0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
	}
	frozenWrongSealKey = []byte{
		0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
		0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
	}
)

type vectorSpec struct {
	name       string
	token      string
	actorKind  string
	actorID    string
	tokenID    string
	wrongKey   bool
	tamper     string
	unreadable bool
	resolves   string
}

type wireVector struct {
	Name       string `json:"name"`
	Token      string `json:"token"`
	KvKey      string `json:"kv_key"`
	ActorKind  string `json:"actor_kind"`
	ActorID    string `json:"actor_id"`
	TokenID    string `json:"token_id"`
	SealedWith string `json:"sealed_with"`
	Corruption string `json:"corruption"`
	Resolves   string `json:"resolves"`
	ValueB64   string `json:"value_b64"`
}

type wireVectors struct {
	Version         int          `json:"version"`
	SealKeyB64      string       `json:"seal_key_b64"`
	WrongSealKeyB64 string       `json:"wrong_seal_key_b64"`
	Vectors         []wireVector `json:"vectors"`
}

func frozenVectorSpecs() []vectorSpec {
	return []vectorSpec{
		{
			name:      "faithful-human",
			token:     "brk_conformance_faithful_human",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0001-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0001-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "faithful-human-second",
			token:     "brk_conformance_faithful_human_second",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0002-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0002-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "faithful-service",
			token:     "brk_conformance_faithful_service",
			actorKind: actorService,
			actorID:   "0190a1b2-0003-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0003-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "revoked",
			token:     "brk_conformance_revoked",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0004-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0004-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "kv-error",
			token:     "brk_conformance_kv_error",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0005-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0005-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "wrong-key",
			token:     "brk_conformance_wrong_key",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0006-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0006-7e5f-8a9b-0c1d2e3f4a5b",
			wrongKey:  true,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "tampered-ciphertext-faithful",
			token:     "brk_conformance_tampered_ciphertext",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0007-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0007-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "tampered-ciphertext-corrupt",
			token:     "brk_conformance_tampered_ciphertext",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0007-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0007-7e5f-8a9b-0c1d2e3f4a5b",
			tamper:    tamperCiphertext,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "tampered-nonce-faithful",
			token:     "brk_conformance_tampered_nonce",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0008-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0008-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "tampered-nonce-corrupt",
			token:     "brk_conformance_tampered_nonce",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0008-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0008-7e5f-8a9b-0c1d2e3f4a5b",
			tamper:    tamperNonce,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "unreadable-faithful",
			token:     "brk_conformance_unreadable",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0009-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0009-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:       "unreadable-corrupt",
			token:      "brk_conformance_unreadable",
			actorKind:  actorHuman,
			actorID:    "0190a1b2-0009-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:    "0190c0de-0009-7e5f-8a9b-0c1d2e3f4a5b",
			unreadable: true,
			resolves:   resolvesAnonymous,
		},
	}
}

func frozenNonce(name string) []byte {
	digest := sha256.Sum256([]byte("identity-passport/vector-nonce/" + name))
	return digest[:chacha20poly1305.NonceSize]
}

func buildWireVectors() (wireVectors, error) {
	specs := frozenVectorSpecs()
	seen := make(map[string]struct{}, len(specs))
	out := wireVectors{
		Version:         wireVectorsVersion,
		SealKeyB64:      base64.StdEncoding.EncodeToString(frozenSealKey),
		WrongSealKeyB64: base64.StdEncoding.EncodeToString(frozenWrongSealKey),
		Vectors:         make([]wireVector, 0, len(specs)),
	}
	for _, spec := range specs {
		if _, dup := seen[spec.name]; dup {
			return wireVectors{}, fmt.Errorf("duplicate vector name %q", spec.name)
		}
		seen[spec.name] = struct{}{}

		key := frozenSealKey
		sealedWith := sealedWithSealKey
		if spec.wrongKey {
			key = frozenWrongSealKey
			sealedWith = sealedWithWrongKey
		}
		result, err := sealOnce(sealRequest{
			key:        key,
			token:      spec.token,
			actor:      spec.actorKind + ":" + spec.actorID,
			tokenID:    spec.tokenID,
			tamper:     spec.tamper,
			unreadable: spec.unreadable,
			nonce:      frozenNonce(spec.name),
		})
		if err != nil {
			return wireVectors{}, fmt.Errorf("sealing vector %q: %w", spec.name, err)
		}
		out.Vectors = append(out.Vectors, wireVector{
			Name:       spec.name,
			Token:      spec.token,
			KvKey:      result.KvKey,
			ActorKind:  spec.actorKind,
			ActorID:    spec.actorID,
			TokenID:    spec.tokenID,
			SealedWith: sealedWith,
			Corruption: corruptionOf(spec),
			Resolves:   spec.resolves,
			ValueB64:   result.ValueB64,
		})
	}
	return out, nil
}

func corruptionOf(spec vectorSpec) string {
	if spec.unreadable {
		return corruptUnreadable
	}
	if spec.tamper == tamperNone {
		return corruptionNone
	}
	return spec.tamper
}

func renderWireVectors() ([]byte, error) {
	vectors, err := buildWireVectors()
	if err != nil {
		return nil, err
	}
	body, err := json.MarshalIndent(vectors, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("marshalling the vector file: %w", err)
	}
	return append(body, '\n'), nil
}

func runVectors(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("vectors", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	var path string
	fs.StringVar(&path, "out", "", "write the vector file here instead of stdout")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if fs.NArg() != 0 {
		return fmt.Errorf("unexpected positional argument %q", fs.Arg(0))
	}
	body, err := renderWireVectors()
	if err != nil {
		return err
	}
	if path == "" {
		_, err = out.Write(body)
		return err
	}
	if err := os.WriteFile(path, body, 0o644); err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}
	return nil
}
