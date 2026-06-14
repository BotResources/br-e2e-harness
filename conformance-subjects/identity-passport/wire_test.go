package main

import (
	"encoding/base64"
	"encoding/json"
	"reflect"
	"testing"

	"github.com/google/uuid"
)

func TestBearerTokenKeyMatchesLibVector(t *testing.T) {
	const token = "brk_test_token_0001"
	const want = "08b6b8ef9b27ca8d4561a519a9ab32cadb11ab16b66e2a47280dc55dccef8fd9"
	got := bearerTokenKey(token)
	if got != want {
		t.Fatalf("bearerTokenKey(%q) = %s, want %s", token, got, want)
	}
	if len(got) != 64 {
		t.Fatalf("key length = %d, want 64", len(got))
	}
}

func TestUserIDFromEmailIsDeterministicV5(t *testing.T) {
	const email = "alice@example.com"
	const want = "ec40195b-2bcc-58bb-b5d3-4db2e505cee5"
	got := userIDFromEmail(email)
	if got != want {
		t.Fatalf("userIDFromEmail(%q) = %s, want %s", email, got, want)
	}
	parsed, err := uuid.Parse(got)
	if err != nil {
		t.Fatalf("user_id is not a valid uuid: %v", err)
	}
	if parsed.Version() != 5 {
		t.Fatalf("user_id version = %d, want 5", parsed.Version())
	}
}

func TestPassportMatchesGoldenShape(t *testing.T) {
	entry := bearerTokenEntry{
		Email:   "alice@example.com",
		TokenID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
	}
	got, err := json.Marshal(passportForEntry(entry))
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	golden := `{
      "kind": "human",
      "user_id": "ec40195b-2bcc-58bb-b5d3-4db2e505cee5",
      "is_super_admin": false,
      "is_active": true,
      "auth_method": {"method": "pat", "token_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"},
      "impersonator": null,
      "claims": {"email": "alice@example.com"}
    }`

	assertJSONEqual(t, got, []byte(golden))
}

func TestPassportTopLevelKeysAreExactlyTheContract(t *testing.T) {
	got, err := json.Marshal(passportForEntry(bearerTokenEntry{Email: "x@y.z", TokenID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}))
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

func TestAuthMethodIsPatWithTokenID(t *testing.T) {
	got, err := json.Marshal(passportForEntry(bearerTokenEntry{Email: "x@y.z", TokenID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}))
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
	if p.AuthMethod["token_id"] != "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b" {
		t.Fatalf("auth_method.token_id = %v, want the entry token_id", p.AuthMethod["token_id"])
	}
	if len(p.AuthMethod) != 2 {
		t.Fatalf("auth_method has %d keys, want exactly method+token_id", len(p.AuthMethod))
	}
}

func TestClaimsIsAnObject(t *testing.T) {
	got, err := json.Marshal(passportForEntry(bearerTokenEntry{Email: "alice@example.com", TokenID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}))
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
	if obj["email"] != "alice@example.com" {
		t.Fatalf("claims.email = %v, want alice@example.com", obj["email"])
	}
}

func TestEncodePassportHeaderIsStandardBase64JSON(t *testing.T) {
	passport := passportForEntry(bearerTokenEntry{Email: "alice@example.com", TokenID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"})
	header, err := encodePassportHeader(passport)
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
