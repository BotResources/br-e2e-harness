package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	frozenEntryPlaintext = `{"actor":{"kind":"human","id":"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},"token_id":"0190c0de-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}`
	frozenServiceActor   = `{"actor":{"kind":"service","id":"0190c0de-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},"token_id":"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}`
)

func fixedKeyB64() string {
	return base64.StdEncoding.EncodeToString(fixedKey())
}

func otherKeyB64() string {
	key := make([]byte, 32)
	for i := range key {
		key[i] = 0x99
	}
	return base64.StdEncoding.EncodeToString(key)
}

func goldenSealArgs() []string {
	return []string{
		"--key", fixedKeyB64(),
		"--token", goldenToken,
		"--actor", "human:" + goldenUserID,
		"--token-id", goldenTokenID,
	}
}

func sealCLI(t *testing.T, args ...string) sealResult {
	t.Helper()
	var out bytes.Buffer
	if err := runSeal(args, &out); err != nil {
		t.Fatalf("runSeal(%v): %v", args, err)
	}
	line := out.String()
	if strings.Count(line, "\n") != 1 || !strings.HasSuffix(line, "\n") {
		t.Fatalf("seal must print exactly one line, got %q", line)
	}
	var result sealResult
	if err := json.Unmarshal([]byte(line), &result); err != nil {
		t.Fatalf("seal output is not one JSON object: %v (%q)", err, line)
	}
	return result
}

func storedValue(t *testing.T, result sealResult) []byte {
	t.Helper()
	raw, err := base64.StdEncoding.DecodeString(result.ValueB64)
	if err != nil {
		t.Fatalf("value_b64 is not base64-std: %v", err)
	}
	return raw
}

func TestSealedPlaintextIsTheFrozenRustEntryByteString(t *testing.T) {
	got, err := json.Marshal(goldenEntry())
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	if string(got) != frozenEntryPlaintext {
		t.Fatalf("sealed cleartext drifted from the frozen entry bytes.\n got: %s\nwant: %s", got, frozenEntryPlaintext)
	}
	service, err := json.Marshal(bearerEntry{
		Actor:   bearerActor{Kind: "service", ID: goldenTokenID},
		TokenID: goldenUserID,
	})
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	if string(service) != frozenServiceActor {
		t.Fatalf("service-actor cleartext drifted.\n got: %s\nwant: %s", service, frozenServiceActor)
	}
}

func TestSealWithTheFrozenNonceReproducesTheFrozenCiphertext(t *testing.T) {
	aead, err := chacha20poly1305.New(fixedKey())
	if err != nil {
		t.Fatalf("new aead: %v", err)
	}
	nonce, err := base64.StdEncoding.DecodeString(goldenNonce)
	if err != nil {
		t.Fatalf("decode golden nonce: %v", err)
	}
	sealed, err := sealEntryWithNonce(aead, goldenToken, goldenEntry(), nonce)
	if err != nil {
		t.Fatalf("sealEntryWithNonce: %v", err)
	}
	if sealed.Nonce != goldenNonce || sealed.Ciphertext != goldenCiphertext {
		t.Fatalf("frozen envelope drifted.\n got: %+v\nwant nonce=%s ciphertext=%s", sealed, goldenNonce, goldenCiphertext)
	}
}

func TestSealedEnvelopeIsTheFrozenTwoKeyJSONInOrder(t *testing.T) {
	value := storedValue(t, sealCLI(t, goldenSealArgs()...))
	var sealed sealedBearer
	if err := json.Unmarshal(value, &sealed); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	want := `{"nonce":"` + sealed.Nonce + `","ciphertext":"` + sealed.Ciphertext + `"}`
	if string(value) != want {
		t.Fatalf("envelope bytes drifted from the frozen shape.\n got: %s\nwant: %s", value, want)
	}
}

func TestSealCLIEmitsTheContractKvKeyAndAnOpenableEnvelope(t *testing.T) {
	result := sealCLI(t, goldenSealArgs()...)
	if result.KvKey != bearerTokensKeyPrefix+goldenDigest {
		t.Fatalf("kv_key = %s, want %s", result.KvKey, bearerTokensKeyPrefix+goldenDigest)
	}
	sealed, err := parseSealed(storedValue(t, result))
	if err != nil {
		t.Fatalf("the sealed value must parse: %v", err)
	}
	aead, _ := chacha20poly1305.New(fixedKey())
	opened, err := openSealed(aead, goldenToken, sealed)
	if err != nil {
		t.Fatalf("the resolver must open what seal produced: %v", err)
	}
	if !reflect.DeepEqual(opened, goldenEntry()) {
		t.Fatalf("opened = %#v, want %#v", opened, goldenEntry())
	}
}

func TestSealCLIRoundTripsAServiceActor(t *testing.T) {
	result := sealCLI(t,
		"--key", fixedKeyB64(),
		"--token", goldenToken,
		"--actor", "service:"+goldenUserID,
		"--token-id", goldenTokenID,
	)
	sealed, err := parseSealed(storedValue(t, result))
	if err != nil {
		t.Fatalf("parseSealed: %v", err)
	}
	aead, _ := chacha20poly1305.New(fixedKey())
	opened, err := openSealed(aead, goldenToken, sealed)
	if err != nil {
		t.Fatalf("openSealed: %v", err)
	}
	want := bearerEntry{Actor: bearerActor{Kind: "service", ID: goldenUserID}, TokenID: goldenTokenID}
	if !reflect.DeepEqual(opened, want) {
		t.Fatalf("opened = %#v, want %#v", opened, want)
	}
}

func TestSealTwiceDrawsDistinctNoncesAndBothOpen(t *testing.T) {
	first := storedValue(t, sealCLI(t, goldenSealArgs()...))
	second := storedValue(t, sealCLI(t, goldenSealArgs()...))
	if bytes.Equal(first, second) {
		t.Fatalf("two seals of the same payload must differ (fresh nonce per seal)")
	}
	aead, _ := chacha20poly1305.New(fixedKey())
	for _, value := range [][]byte{first, second} {
		sealed, err := parseSealed(value)
		if err != nil {
			t.Fatalf("parseSealed: %v", err)
		}
		if _, err := openSealed(aead, goldenToken, sealed); err != nil {
			t.Fatalf("openSealed: %v", err)
		}
	}
}

func TestSealUnderAWrongKeyDoesNotOpenWithTheResolverKey(t *testing.T) {
	result := sealCLI(t,
		"--key", otherKeyB64(),
		"--token", goldenToken,
		"--actor", "human:"+goldenUserID,
		"--token-id", goldenTokenID,
	)
	if result.KvKey != bearerTokensKeyPrefix+goldenDigest {
		t.Fatalf("the kv key must not depend on the seal key, got %s", result.KvKey)
	}
	sealed, err := parseSealed(storedValue(t, result))
	if err != nil {
		t.Fatalf("a wrong-key envelope must still parse: %v", err)
	}
	aead, _ := chacha20poly1305.New(fixedKey())
	if _, err := openSealed(aead, goldenToken, sealed); err == nil {
		t.Fatalf("an envelope sealed under another key must not open")
	}
}

func TestTamperedSealsParseButNeverOpen(t *testing.T) {
	aead, _ := chacha20poly1305.New(fixedKey())
	for _, mode := range []string{tamperCiphertext, tamperNonce} {
		t.Run(mode, func(t *testing.T) {
			result := sealCLI(t, append(goldenSealArgs(), "--tamper", mode)...)
			sealed, err := parseSealed(storedValue(t, result))
			if err != nil {
				t.Fatalf("a tampered envelope must still parse (the failure must be the AEAD): %v", err)
			}
			if _, err := openSealed(aead, goldenToken, sealed); err == nil {
				t.Fatalf("a tampered %s must fail the AEAD", mode)
			}
		})
	}
}

func TestUnreadableSealIsRejectedByTheParserNotTheAead(t *testing.T) {
	result := sealCLI(t, append(goldenSealArgs(), "--unreadable")...)
	value := storedValue(t, result)
	if _, err := parseSealed(value); err == nil {
		t.Fatalf("an --unreadable envelope must be rejected by parseSealed")
	}
	var loose sealedBearer
	if err := json.Unmarshal(value, &loose); err != nil {
		t.Fatalf("the unreadable envelope must still be JSON carrying a real seal: %v", err)
	}
	aead, _ := chacha20poly1305.New(fixedKey())
	opened, err := openSealed(aead, goldenToken, loose)
	if err != nil {
		t.Fatalf("the unreadable case must be unreadable ONLY because of the unknown field: %v", err)
	}
	if !reflect.DeepEqual(opened, goldenEntry()) {
		t.Fatalf("opened = %#v, want %#v", opened, goldenEntry())
	}
}

func TestSealRejectsBadInput(t *testing.T) {
	cases := []struct {
		name string
		args []string
	}{
		{"no key", []string{"--token", goldenToken, "--actor", "human:" + goldenUserID, "--token-id", goldenTokenID}},
		{"no token", []string{"--key", fixedKeyB64(), "--actor", "human:" + goldenUserID, "--token-id", goldenTokenID}},
		{"no actor", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--token-id", goldenTokenID}},
		{"no token id", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--actor", "human:" + goldenUserID}},
		{"key not base64", append(goldenSealArgs()[2:], "--key", "not base64!")},
		{"key wrong length", append(goldenSealArgs()[2:], "--key", base64.StdEncoding.EncodeToString([]byte("short")))},
		{"actor kind unknown", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--actor", "robot:" + goldenUserID, "--token-id", goldenTokenID}},
		{"actor id not uuid", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--actor", "human:nope", "--token-id", goldenTokenID}},
		{"actor missing colon", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--actor", "human", "--token-id", goldenTokenID}},
		{"token id not uuid", []string{"--key", fixedKeyB64(), "--token", goldenToken, "--actor", "human:" + goldenUserID, "--token-id", "nope"}},
		{"tamper mode unknown", append(goldenSealArgs(), "--tamper", "everything")},
		{"tamper with unreadable", append(goldenSealArgs(), "--tamper", tamperCiphertext, "--unreadable")},
		{"unknown flag", append(goldenSealArgs(), "--nonce", goldenNonce)},
		{"positional argument", append(goldenSealArgs(), "extra")},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var out bytes.Buffer
			if err := runSeal(tc.args, &out); err == nil {
				t.Fatalf("runSeal(%v) must fail", tc.args)
			}
			if out.Len() != 0 {
				t.Fatalf("a rejected seal must print nothing on stdout, got %q", out.String())
			}
		})
	}
}
