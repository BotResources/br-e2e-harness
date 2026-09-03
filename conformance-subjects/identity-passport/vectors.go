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
	mutationNone       = ""
	resolvesHuman      = "human"
	resolvesAnonymous  = "anonymous"
	resolvesUnasserted = "unasserted"
	wireVectorsVersion = 1
)

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

func frozenNonce(token string) []byte {
	digest := sha256.Sum256([]byte("identity-passport/vector-nonce/" + token))
	return digest[:chacha20poly1305.NonceSize]
}

func buildWireVectors() (wireVectors, error) {
	specs := frozenVectorSpecs()
	built := make(map[string]wireVector, len(specs))
	out := wireVectors{
		Version:         wireVectorsVersion,
		SealKeyB64:      base64.StdEncoding.EncodeToString(frozenSealKey),
		WrongSealKeyB64: base64.StdEncoding.EncodeToString(frozenWrongSealKey),
		Vectors:         make([]wireVector, 0, len(specs)),
	}
	for _, spec := range specs {
		if _, dup := built[spec.name]; dup {
			return wireVectors{}, fmt.Errorf("duplicate vector name %q", spec.name)
		}
		vector, err := buildVector(spec, built)
		if err != nil {
			return wireVectors{}, err
		}
		built[spec.name] = vector
		out.Vectors = append(out.Vectors, vector)
	}
	return out, nil
}

func buildVector(spec vectorSpec, built map[string]wireVector) (wireVector, error) {
	if spec.mutation != mutationNone {
		return buildTwinVector(spec, built)
	}
	if spec.twinOf != "" {
		return wireVector{}, fmt.Errorf("vector %q declares a twin but no mutation", spec.name)
	}
	key := frozenSealKey
	sealedWith := sealedWithSealKey
	if spec.wrongKey {
		key = frozenWrongSealKey
		sealedWith = sealedWithWrongKey
	}
	result, err := sealOnce(sealRequest{
		key:     key,
		token:   spec.token,
		actor:   spec.actorKind + ":" + spec.actorID,
		tokenID: spec.tokenID,
		nonce:   frozenNonce(spec.token),
	})
	if err != nil {
		return wireVector{}, fmt.Errorf("sealing vector %q: %w", spec.name, err)
	}
	return renderVector(spec, sealedWith, result.KvKey, result.ValueB64), nil
}

func buildTwinVector(spec vectorSpec, built map[string]wireVector) (wireVector, error) {
	faithful, ok := built[spec.twinOf]
	if !ok {
		return wireVector{}, fmt.Errorf("vector %q names an unbuilt twin %q", spec.name, spec.twinOf)
	}
	if err := assertTwinIdentity(spec, faithful); err != nil {
		return wireVector{}, err
	}
	value, err := base64.StdEncoding.DecodeString(faithful.ValueB64)
	if err != nil {
		return wireVector{}, fmt.Errorf("vector %q: twin %q is not base64-std: %w", spec.name, spec.twinOf, err)
	}
	mutated, err := mutateStoredValue(value, spec.mutation)
	if err != nil {
		return wireVector{}, fmt.Errorf("vector %q: %w", spec.name, err)
	}
	return renderVector(spec, faithful.SealedWith, faithful.KvKey, base64.StdEncoding.EncodeToString(mutated)), nil
}

func assertTwinIdentity(spec vectorSpec, faithful wireVector) error {
	if faithful.Corruption != corruptionNone {
		return fmt.Errorf("vector %q must be the twin of a faithful vector, %q is %s", spec.name, faithful.Name, faithful.Corruption)
	}
	if spec.wrongKey {
		return fmt.Errorf("vector %q cannot both be a twin and be sealed under the wrong key", spec.name)
	}
	sameIdentity := spec.token == faithful.Token &&
		spec.actorKind == faithful.ActorKind &&
		spec.actorID == faithful.ActorID &&
		spec.tokenID == faithful.TokenID
	if !sameIdentity {
		return fmt.Errorf("vector %q must carry the exact identity of its twin %q", spec.name, faithful.Name)
	}
	return nil
}

func renderVector(spec vectorSpec, sealedWith, kvKey, valueB64 string) wireVector {
	corruption := corruptionNone
	if spec.mutation != mutationNone {
		corruption = spec.mutation
	}
	return wireVector{
		Name:       spec.name,
		Token:      spec.token,
		KvKey:      kvKey,
		ActorKind:  spec.actorKind,
		ActorID:    spec.actorID,
		TokenID:    spec.tokenID,
		SealedWith: sealedWith,
		Corruption: corruption,
		Resolves:   spec.resolves,
		ValueB64:   valueB64,
	}
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
