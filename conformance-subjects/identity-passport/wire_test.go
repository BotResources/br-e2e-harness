package main

import (
	"encoding/base64"
	"encoding/json"
	"reflect"
	"testing"

	"golang.org/x/crypto/chacha20poly1305"
)

const (
	goldenToken      = "brk_test_token_0001"
	goldenDigest     = "08b6b8ef9b27ca8d4561a519a9ab32cadb11ab16b66e2a47280dc55dccef8fd9"
	goldenUserID     = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
	goldenTokenID    = "0190c0de-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
	goldenNonce      = "AAECAwQFBgcICQoL"
	goldenCiphertext = "5Z9T88UIStuvgnzPf62Y4s1Y4K5gukTGNOT3YbWYTSvpnju/s8WS+5USRn2zOefu6/uNFaKNRo6jpWjP6Mb3W1qReMiIp9ZBIJKSQfBmGCYf3d935nfG/5vzJ+wnp/5Ko77Fy69/pPkTKJlC8vDgacyvJLS46qyb9ovWAiwhKe8rDoYpZNzGKg=="
)

func goldenEntry() bearerEntry {
	return bearerEntry{
		Actor:   bearerActor{Kind: "human", ID: goldenUserID},
		TokenID: goldenTokenID,
	}
}

func fixedKey() []byte {
	key := make([]byte, 32)
	for i := range key {
		key[i] = 0x2a
	}
	return key
}

func TestKvKeyIsPrefixPlusUnstrippedSha256(t *testing.T) {
	want := bearerTokensKeyPrefix + goldenDigest
	got := kvKey(goldenToken)
	if got != want {
		t.Fatalf("kvKey(%q) = %s, want %s", goldenToken, got, want)
	}
	if len(sha256Hex(goldenToken)) != 64 {
		t.Fatalf("digest length = %d, want 64", len(sha256Hex(goldenToken)))
	}
}

func TestAadIsTheUnprefixedDigestNotTheKvKey(t *testing.T) {
	if string(aad(goldenToken)) != goldenDigest {
		t.Fatalf("aad(%q) = %q, want the bare digest %q", goldenToken, string(aad(goldenToken)), goldenDigest)
	}
	if string(aad(goldenToken)) == kvKey(goldenToken) {
		t.Fatalf("aad must be the unprefixed digest, never the full kv key %q", kvKey(goldenToken))
	}
}

func TestSealOpenRoundTripWithFixedNonceFreezesTheAead(t *testing.T) {
	aead, err := chacha20poly1305.New(fixedKey())
	if err != nil {
		t.Fatalf("new aead: %v", err)
	}
	nonce, err := base64.StdEncoding.DecodeString(goldenNonce)
	if err != nil {
		t.Fatalf("decode golden nonce: %v", err)
	}
	plaintext, err := json.Marshal(goldenEntry())
	if err != nil {
		t.Fatalf("marshal entry: %v", err)
	}
	ct := aead.Seal(nil, nonce, plaintext, aad(goldenToken))
	if got := base64.StdEncoding.EncodeToString(ct); got != goldenCiphertext {
		t.Fatalf("frozen ciphertext drifted.\n got: %s\nwant: %s", got, goldenCiphertext)
	}

	sealed := sealedBearer{Nonce: goldenNonce, Ciphertext: goldenCiphertext}
	opened, err := openSealed(aead, goldenToken, sealed)
	if err != nil {
		t.Fatalf("openSealed of the frozen envelope: %v", err)
	}
	if !reflect.DeepEqual(opened, goldenEntry()) {
		t.Fatalf("opened entry = %#v, want %#v", opened, goldenEntry())
	}
}

func TestOpenUnderTheWrongTokenFailsClosed(t *testing.T) {
	aead, _ := chacha20poly1305.New(fixedKey())
	sealed := sealedBearer{Nonce: goldenNonce, Ciphertext: goldenCiphertext}
	if _, err := openSealed(aead, "brk_some_other_token", sealed); err == nil {
		t.Fatalf("opening with a different token (AAD) must fail: the token is the sole AAD")
	}
}

func TestOpenWithTamperedCiphertextFailsClosed(t *testing.T) {
	aead, _ := chacha20poly1305.New(fixedKey())
	raw, _ := base64.StdEncoding.DecodeString(goldenCiphertext)
	raw[0] ^= 0xff
	sealed := sealedBearer{Nonce: goldenNonce, Ciphertext: base64.StdEncoding.EncodeToString(raw)}
	if _, err := openSealed(aead, goldenToken, sealed); err == nil {
		t.Fatalf("opening a tampered ciphertext must fail the AEAD tag")
	}
}

func TestSealedBearerRejectsUnknownField(t *testing.T) {
	raw := []byte(`{"nonce":"AAECAwQFBgcICQoL","ciphertext":"AA==","evil":true}`)
	if _, err := parseSealed(raw); err == nil {
		t.Fatalf("parseSealed must reject an unknown field (mirrors deny_unknown_fields)")
	}
}

func TestPassportMatchesGoldenShape(t *testing.T) {
	got, err := json.Marshal(passportForEntry(goldenEntry()))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{
      "kind": "human",
      "user_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
      "is_super_admin": false,
      "is_active": true,
      "auth_method": {"method": "pat", "token_id": "0190c0de-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},
      "impersonator": null,
      "claims": {}
    }`
	assertJSONEqual(t, got, []byte(golden))
}

func TestUserIDIsTheActorIdNotDerivedFromEmail(t *testing.T) {
	p := passportForEntry(goldenEntry())
	if p.UserID != goldenUserID {
		t.Fatalf("user_id = %s, want the actor id %s (no email derivation)", p.UserID, goldenUserID)
	}
}

func TestPassportTopLevelKeysAreExactlyTheContract(t *testing.T) {
	got, err := json.Marshal(passportForEntry(goldenEntry()))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(got, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	want := map[string]struct{}{
		"kind": {}, "user_id": {}, "is_super_admin": {}, "is_active": {},
		"auth_method": {}, "impersonator": {}, "claims": {},
	}
	for k := range asMap {
		if _, ok := want[k]; !ok {
			t.Fatalf("unexpected top-level key %q (deny_unknown_fields would reject it)", k)
		}
	}
	for k := range want {
		if _, ok := asMap[k]; !ok {
			t.Fatalf("missing required top-level key %q", k)
		}
	}
}

func TestClaimsIsAnEmptyObjectWithNoEmail(t *testing.T) {
	got, err := json.Marshal(passportForEntry(goldenEntry()))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var p struct {
		Claims json.RawMessage `json:"claims"`
	}
	if err := json.Unmarshal(got, &p); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	var obj map[string]any
	if err := json.Unmarshal(p.Claims, &obj); err != nil {
		t.Fatalf("claims must be a JSON object: %v", err)
	}
	if len(obj) != 0 {
		t.Fatalf("claims must be empty (no email in the sealed model), got %v", obj)
	}
}

func TestAuthMethodIsPatWithTokenID(t *testing.T) {
	got, err := json.Marshal(passportForEntry(goldenEntry()))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var p struct {
		AuthMethod map[string]any `json:"auth_method"`
	}
	if err := json.Unmarshal(got, &p); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if p.AuthMethod["method"] != "pat" {
		t.Fatalf("auth_method.method = %v, want pat", p.AuthMethod["method"])
	}
	if p.AuthMethod["token_id"] != goldenTokenID {
		t.Fatalf("auth_method.token_id = %v, want %s", p.AuthMethod["token_id"], goldenTokenID)
	}
	if len(p.AuthMethod) != 2 {
		t.Fatalf("auth_method has %d keys, want exactly method+token_id", len(p.AuthMethod))
	}
}

func TestEncodePassportHeaderIsStandardBase64JSON(t *testing.T) {
	header, err := encodePassportHeader(passportForEntry(goldenEntry()))
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	decoded, err := base64.StdEncoding.DecodeString(header)
	if err != nil {
		t.Fatalf("header is not standard base64: %v", err)
	}
	var roundtrip humanPassport
	if err := json.Unmarshal(decoded, &roundtrip); err != nil {
		t.Fatalf("decoded header is not the passport JSON: %v", err)
	}
	if roundtrip.Kind != "human" {
		t.Fatalf("decoded kind = %q, want human", roundtrip.Kind)
	}
}

func TestBearerTokenParsing(t *testing.T) {
	cases := []struct {
		name      string
		header    string
		wantToken string
		wantOK    bool
	}{
		{"valid", "Bearer brk_abc", "brk_abc", true},
		{"empty header", "", "", false},
		{"not bearer", "Basic abc", "", false},
		{"bearer no token", "Bearer ", "", false},
		{"bearer only", "Bearer", "", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			token, ok := bearerToken(tc.header)
			if ok != tc.wantOK || token != tc.wantToken {
				t.Fatalf("bearerToken(%q) = (%q, %v), want (%q, %v)", tc.header, token, ok, tc.wantToken, tc.wantOK)
			}
		})
	}
}

func assertJSONEqual(t *testing.T, a, b []byte) {
	t.Helper()
	var am, bm any
	if err := json.Unmarshal(a, &am); err != nil {
		t.Fatalf("unmarshal got: %v\n%s", err, a)
	}
	if err := json.Unmarshal(b, &bm); err != nil {
		t.Fatalf("unmarshal golden: %v", err)
	}
	if !reflect.DeepEqual(am, bm) {
		ap, _ := json.MarshalIndent(am, "", "  ")
		bp, _ := json.MarshalIndent(bm, "", "  ")
		t.Fatalf("JSON mismatch.\n--- got ---\n%s\n--- want ---\n%s", ap, bp)
	}
}
