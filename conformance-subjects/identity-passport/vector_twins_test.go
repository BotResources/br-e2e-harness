package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"reflect"
	"testing"
)

func frozenTwinPairs() [][2]string {
	specs := frozenVectorSpecs()
	pairs := make([][2]string, 0, len(specs))
	for _, spec := range specs {
		if spec.twinOf != "" {
			pairs = append(pairs, [2]string{spec.twinOf, spec.name})
		}
	}
	return pairs
}

func decodeVectorValue(t *testing.T, v wireVector) []byte {
	t.Helper()
	raw, err := base64.StdEncoding.DecodeString(v.ValueB64)
	if err != nil {
		t.Fatalf("%s: value_b64 is not base64-std: %v", v.Name, err)
	}
	return raw
}

func looseEnvelope(t *testing.T, v wireVector) sealedBearer {
	t.Helper()
	var loose sealedBearer
	if err := json.Unmarshal(decodeVectorValue(t, v), &loose); err != nil {
		t.Fatalf("%s: the stored value is not an envelope: %v", v.Name, err)
	}
	return loose
}

func decodedField(t *testing.T, name, field, b64 string) []byte {
	t.Helper()
	raw, err := base64.StdEncoding.DecodeString(b64)
	if err != nil {
		t.Fatalf("%s: %s is not base64-std: %v", name, field, err)
	}
	return raw
}

func TestEveryCorruptVectorIsItsFaithfulTwinPlusExactlyTheDeclaredMutation(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	pairs := frozenTwinPairs()
	if len(pairs) == 0 {
		t.Fatalf("the frozen set must carry corrupted twins")
	}
	for _, pair := range pairs {
		faithful := vectorByName(t, parsed, pair[0])
		corrupt := vectorByName(t, parsed, pair[1])
		t.Run(corrupt.Name, func(t *testing.T) {
			assertTwinsCarryOneIdentity(t, faithful, corrupt)
			mutated, err := mutateStoredValue(decodeVectorValue(t, faithful), corrupt.Corruption)
			if err != nil {
				t.Fatalf("applying %q to the faithful envelope: %v", corrupt.Corruption, err)
			}
			if !bytes.Equal(mutated, decodeVectorValue(t, corrupt)) {
				t.Fatalf("%s is not %s with %q applied — regenerate with `make vectors`", corrupt.Name, faithful.Name, corrupt.Corruption)
			}
			assertOnlyTheDeclaredMutationDiffers(t, faithful, corrupt)
		})
	}
}

func assertTwinsCarryOneIdentity(t *testing.T, faithful, corrupt wireVector) {
	t.Helper()
	if faithful.Corruption != corruptionNone {
		t.Fatalf("%s must be faithful, it declares %q", faithful.Name, faithful.Corruption)
	}
	if corrupt.Corruption == corruptionNone {
		t.Fatalf("%s must declare its mutation", corrupt.Name)
	}
	if faithful.Token != corrupt.Token || faithful.KvKey != corrupt.KvKey {
		t.Fatalf("a twin pair must share one token and one kv key")
	}
	if faithful.SealedWith != corrupt.SealedWith {
		t.Fatalf("a twin pair must be sealed under one key")
	}
	if faithful.ActorKind != corrupt.ActorKind || faithful.ActorID != corrupt.ActorID || faithful.TokenID != corrupt.TokenID {
		t.Fatalf("a twin pair must carry one identity")
	}
	if faithful.ValueB64 == corrupt.ValueB64 {
		t.Fatalf("the corrupted half must differ from the faithful one")
	}
}

func assertOnlyTheDeclaredMutationDiffers(t *testing.T, faithful, corrupt wireVector) {
	t.Helper()
	switch corrupt.Corruption {
	case tamperCiphertext:
		assertOneByteFlipped(t, corrupt.Name, "ciphertext", faithful, corrupt)
	case tamperNonce:
		assertOneByteFlipped(t, corrupt.Name, "nonce", faithful, corrupt)
	case corruptUnreadable:
		assertOnlyAnUnknownKeyWasAdded(t, faithful, corrupt)
	default:
		t.Fatalf("%s declares an unknown mutation %q", corrupt.Name, corrupt.Corruption)
	}
}

func assertOneByteFlipped(t *testing.T, name, field string, faithful, corrupt wireVector) {
	t.Helper()
	before := looseEnvelope(t, faithful)
	after := looseEnvelope(t, corrupt)
	mutatedBefore, mutatedAfter := before.Ciphertext, after.Ciphertext
	untouchedBefore, untouchedAfter := before.Nonce, after.Nonce
	untouchedField := "nonce"
	if field == "nonce" {
		mutatedBefore, mutatedAfter = before.Nonce, after.Nonce
		untouchedBefore, untouchedAfter = before.Ciphertext, after.Ciphertext
		untouchedField = "ciphertext"
	}
	if untouchedBefore != untouchedAfter {
		t.Fatalf("%s: the %s must be byte-identical to its twin's", name, untouchedField)
	}
	from := decodedField(t, faithful.Name, field, mutatedBefore)
	to := decodedField(t, name, field, mutatedAfter)
	if len(from) != len(to) {
		t.Fatalf("%s: the %s changed length (%d → %d), a flip keeps it", name, field, len(from), len(to))
	}
	var differing []int
	for i := range from {
		if from[i] != to[i] {
			differing = append(differing, i)
		}
	}
	if len(differing) != 1 || differing[0] != 0 {
		t.Fatalf("%s: bytes %v of the %s differ, the declared mutation flips byte 0 alone", name, differing, field)
	}
	if to[0] != from[0]^0xff {
		t.Fatalf("%s: byte 0 of the %s is not its faithful byte xor 0xff", name, field)
	}
}

func assertOnlyAnUnknownKeyWasAdded(t *testing.T, faithful, corrupt wireVector) {
	t.Helper()
	var before, after map[string]any
	if err := json.Unmarshal(decodeVectorValue(t, faithful), &before); err != nil {
		t.Fatalf("%s: %v", faithful.Name, err)
	}
	if err := json.Unmarshal(decodeVectorValue(t, corrupt), &after); err != nil {
		t.Fatalf("%s: %v", corrupt.Name, err)
	}
	var extra []string
	for key := range after {
		if _, known := before[key]; !known {
			extra = append(extra, key)
		}
	}
	if len(extra) != 1 {
		t.Fatalf("%s must add exactly one unknown key, it adds %v", corrupt.Name, extra)
	}
	delete(after, extra[0])
	if !reflect.DeepEqual(before, after) {
		t.Fatalf("%s must be %s plus the unknown key %q and nothing else", corrupt.Name, faithful.Name, extra[0])
	}
}

func TestTheGeneratorRefusesATwinThatIsNotTheSameIdentity(t *testing.T) {
	built := map[string]wireVector{
		"faithful-human": {
			Name:       "faithful-human",
			Token:      "brk_conformance_faithful_human",
			ActorKind:  actorHuman,
			ActorID:    "0190a1b2-0001-7e5f-8a9b-0c1d2e3f4a5b",
			TokenID:    "0190c0de-0001-7e5f-8a9b-0c1d2e3f4a5b",
			Corruption: corruptionNone,
		},
	}
	foreign := vectorSpec{
		name:      "foreign-corrupt",
		token:     "brk_conformance_other",
		actorKind: actorHuman,
		actorID:   "0190a1b2-0001-7e5f-8a9b-0c1d2e3f4a5b",
		tokenID:   "0190c0de-0001-7e5f-8a9b-0c1d2e3f4a5b",
		twinOf:    "faithful-human",
		mutation:  tamperCiphertext,
	}
	if _, err := buildTwinVector(foreign, built); err == nil {
		t.Fatalf("a twin carrying another token must be refused")
	}
	unbuilt := foreign
	unbuilt.token = "brk_conformance_faithful_human"
	unbuilt.twinOf = "nowhere"
	if _, err := buildTwinVector(unbuilt, built); err == nil {
		t.Fatalf("a twin naming an unbuilt vector must be refused")
	}
}

func TestMutatingRequiresAFaithfullyParsableEnvelope(t *testing.T) {
	if _, err := mutateStoredValue([]byte(`{"nonce":"AA==","ciphertext":"AA==","evil":true}`), tamperNonce); err == nil {
		t.Fatalf("mutating an already-unreadable envelope must be refused")
	}
	if _, err := mutateStoredValue([]byte(`{"nonce":"AA==","ciphertext":"AA=="}`), "shredded"); err == nil {
		t.Fatalf("an unknown mutation must be refused")
	}
}

func TestEveryCorruptVectorIsNamedByATwinPair(t *testing.T) {
	parsed := decodeVectors(t, committedVectors(t))
	corruptHalves := make(map[string]struct{}, len(parsed.Vectors))
	for _, pair := range frozenTwinPairs() {
		corruptHalves[pair[1]] = struct{}{}
	}
	for _, v := range parsed.Vectors {
		_, twinned := corruptHalves[v.Name]
		if v.Corruption != corruptionNone && !twinned {
			t.Fatalf("vector %q declares corruption %q but no twin pair names it", v.Name, v.Corruption)
		}
		if v.Corruption == corruptionNone && twinned {
			t.Fatalf("vector %q is the corrupt half of a pair but declares no corruption", v.Name)
		}
	}
}
