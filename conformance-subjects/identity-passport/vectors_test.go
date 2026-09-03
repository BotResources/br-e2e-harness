package main

import (
	"bytes"
	"crypto/cipher"
	"encoding/base64"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"golang.org/x/crypto/chacha20poly1305"
)

const committedVectorsPath = "../../crates/conformance-passport/vectors/passport-wire-v1.json"

func committedVectors(t *testing.T) []byte {
	t.Helper()
	body, err := os.ReadFile(filepath.Clean(committedVectorsPath))
	if err != nil {
		t.Fatalf("the committed vector file must exist (run `make vectors`): %v", err)
	}
	return body
}

func decodeVectors(t *testing.T, body []byte) wireVectors {
	t.Helper()
	var parsed wireVectors
	if err := json.Unmarshal(body, &parsed); err != nil {
		t.Fatalf("the vector file is not valid JSON: %v", err)
	}
	return parsed
}

func vectorByName(t *testing.T, parsed wireVectors, name string) wireVector {
	t.Helper()
	for _, v := range parsed.Vectors {
		if v.Name == name {
			return v
		}
	}
	t.Fatalf("vector %q is missing from the frozen set", name)
	return wireVector{}
}

func aeadFor(t *testing.T, keyB64 string) cipher.AEAD {
	t.Helper()
	key, err := base64.StdEncoding.DecodeString(keyB64)
	if err != nil {
		t.Fatalf("key is not base64-std: %v", err)
	}
	aead, err := chacha20poly1305.New(key)
	if err != nil {
		t.Fatalf("new aead: %v", err)
	}
	return aead
}

func TestCommittedVectorsAreExactlyWhatTheAnchorRegenerates(t *testing.T) {
	regenerated, err := renderWireVectors()
	if err != nil {
		t.Fatalf("regenerating the vectors: %v", err)
	}
	if !bytes.Equal(regenerated, committedVectors(t)) {
		t.Fatalf("the committed vector file drifted from the anchor.\nRun `make vectors` and review the diff — a hand edit of the JSON is never allowed.")
	}
}

func TestRegenerationIsDeterministic(t *testing.T) {
	first, err := renderWireVectors()
	if err != nil {
		t.Fatalf("render: %v", err)
	}
	second, err := renderWireVectors()
	if err != nil {
		t.Fatalf("render: %v", err)
	}
	if !bytes.Equal(first, second) {
		t.Fatalf("two renders differ: the generator must be deterministic (fixed keys, fixed nonces)")
	}
}

func TestEveryFaithfulVectorOpensAndCarriesItsDeclaredIdentity(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	aead := aeadFor(t, parsed.SealKeyB64)
	for _, v := range parsed.Vectors {
		if v.SealedWith != sealedWithSealKey || v.Corruption != corruptionNone {
			continue
		}
		t.Run(v.Name, func(t *testing.T) {
			raw, err := base64.StdEncoding.DecodeString(v.ValueB64)
			if err != nil {
				t.Fatalf("value_b64: %v", err)
			}
			sealed, err := parseSealed(raw)
			if err != nil {
				t.Fatalf("a faithful vector must parse: %v", err)
			}
			opened, err := openSealed(aead, v.Token, sealed)
			if err != nil {
				t.Fatalf("a faithful vector must open under the seal key: %v", err)
			}
			want := bearerEntry{
				Actor:   bearerActor{Kind: v.ActorKind, ID: v.ActorID},
				TokenID: v.TokenID,
			}
			if !reflect.DeepEqual(opened, want) {
				t.Fatalf("opened = %#v, want %#v", opened, want)
			}
			switch {
			case v.ActorKind == actorHuman && v.Resolves != resolvesHuman:
				t.Fatalf("a faithful human vector must declare resolves=%s, got %s", resolvesHuman, v.Resolves)
			case v.ActorKind != actorHuman && v.Resolves != resolvesUnasserted:
				t.Fatalf("a non-human faithful vector must declare resolves=%s (the battery freezes no resolution policy for it), got %s", resolvesUnasserted, v.Resolves)
			}
		})
	}
}

func TestEveryAnonymousVectorNeverYieldsAnEntry(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	aead := aeadFor(t, parsed.SealKeyB64)
	anonymous := 0
	for _, v := range parsed.Vectors {
		if v.Resolves != resolvesAnonymous {
			continue
		}
		anonymous++
		t.Run(v.Name, func(t *testing.T) {
			raw, err := base64.StdEncoding.DecodeString(v.ValueB64)
			if err != nil {
				t.Fatalf("value_b64: %v", err)
			}
			sealed, err := parseSealed(raw)
			if err != nil {
				if v.Corruption != corruptUnreadable {
					t.Fatalf("only an unreadable vector may fail the parse, %q did: %v", v.Name, err)
				}
				return
			}
			if v.Corruption == corruptUnreadable {
				t.Fatalf("an unreadable vector must be rejected by the parser")
			}
			if _, err := openSealed(aead, v.Token, sealed); err == nil {
				t.Fatalf("an anonymous vector must never open under the seal key")
			}
		})
	}
	if anonymous == 0 {
		t.Fatalf("the frozen set must carry adversarial vectors")
	}
}

func TestUnreadableVectorWouldOpenWithoutItsUnknownField(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	v := vectorByName(t, parsed, "unreadable-corrupt")
	raw, err := base64.StdEncoding.DecodeString(v.ValueB64)
	if err != nil {
		t.Fatalf("value_b64: %v", err)
	}
	var loose sealedBearer
	if err := json.Unmarshal(raw, &loose); err != nil {
		t.Fatalf("the unreadable vector must still be JSON carrying a real seal: %v", err)
	}
	if _, err := openSealed(aeadFor(t, parsed.SealKeyB64), v.Token, loose); err != nil {
		t.Fatalf("the unreadable vector must be unreadable ONLY because of the unknown field: %v", err)
	}
}

func TestDistinctTokensHaveDistinctNoncesAndTwinsShareTheirs(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	nonceByToken := make(map[string]string, len(parsed.Vectors))
	tokenByNonce := make(map[string]string, len(parsed.Vectors))
	for _, v := range parsed.Vectors {
		if v.KvKey != kvKey(v.Token) {
			t.Fatalf("vector %q carries a kv key that is not the digest of its token", v.Name)
		}
		nonce := sealingNonce(t, v)
		if previous, seen := nonceByToken[v.Token]; seen && previous != nonce {
			t.Fatalf("token %q was sealed under two nonces: twins must share theirs", v.Token)
		}
		nonceByToken[v.Token] = nonce
		if previous, seen := tokenByNonce[nonce]; seen && previous != v.Token {
			t.Fatalf("distinct tokens %q and %q reuse a nonce", previous, v.Token)
		}
		tokenByNonce[nonce] = v.Token
	}
	if len(nonceByToken) != len(tokenByNonce) {
		t.Fatalf("the token/nonce mapping is not one-to-one")
	}
}

func sealingNonce(t *testing.T, v wireVector) string {
	t.Helper()
	nonce := looseEnvelope(t, v).Nonce
	if v.Corruption != tamperNonce {
		return nonce
	}
	restored, err := flipFirstByte(nonce)
	if err != nil {
		t.Fatalf("%s: undoing the nonce flip: %v", v.Name, err)
	}
	return restored
}

func TestOnlyTheServiceActorIsLeftUnasserted(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	for _, v := range parsed.Vectors {
		if v.Resolves == resolvesUnasserted && v.ActorKind == actorHuman {
			t.Fatalf("vector %q is a human actor: its resolution must be asserted", v.Name)
		}
		if v.ActorKind == actorService && v.Resolves != resolvesUnasserted {
			t.Fatalf("vector %q is a service actor: the battery must not freeze its resolution", v.Name)
		}
	}
}

func TestVectorFileDeclaresBothFrozenKeys(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	if parsed.Version != wireVectorsVersion {
		t.Fatalf("version = %d, want %d", parsed.Version, wireVectorsVersion)
	}
	if parsed.SealKeyB64 != base64.StdEncoding.EncodeToString(frozenSealKey) {
		t.Fatalf("seal_key_b64 drifted")
	}
	if parsed.WrongSealKeyB64 != base64.StdEncoding.EncodeToString(frozenWrongSealKey) {
		t.Fatalf("wrong_seal_key_b64 drifted")
	}
	if parsed.SealKeyB64 == parsed.WrongSealKeyB64 {
		t.Fatalf("the wrong key must differ from the seal key")
	}
}
